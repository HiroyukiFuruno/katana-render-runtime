use super::super::super::{HtmlFontRequest, discovery_cache, lazy_parse, raster_observer, sources};
use super::{SVG, TEXT};
use crate::markdown::svg_rasterize::{SvgRasterizeOps, text_shaping};
use resvg::usvg::{self, fontdb::Source};
use std::sync::{Arc, Weak};

type WeakBytes = Weak<dyn AsRef<[u8]> + Send + Sync>;

const NORMAL_WEIGHT: u16 = 400;

#[test]
fn lazy_sharedfile_tree_and_clone_keep_maps_until_final_tree_drop() -> Result<(), String> {
    let observer = raster_observer::Scope::enter();
    let tree = lazy_parse::parse(SVG, SVG).map_err(|error| error.to_string())?;
    assert_known_shared_route(&observer.snapshot());
    let database = Arc::downgrade(tree.fontdb());
    let maps = mapped_weaks(&tree)?;
    let retained = tree.clone();
    assert!(Arc::ptr_eq(tree.fontdb(), retained.fontdb()));
    drop(tree);
    assert!(database.upgrade().is_some());
    assert!(maps.iter().all(|data| data.upgrade().is_some()));
    assert_real_render(&retained)?;
    assert_persistent_memo_is_file()?;
    drop(retained);
    assert!(database.upgrade().is_none());
    assert!(maps.iter().all(|data| data.upgrade().is_none()));
    assert_persistent_memo_is_file()?;
    Ok(())
}

fn mapped_weaks(tree: &usvg::Tree) -> Result<Vec<WeakBytes>, String> {
    let maps = tree
        .fontdb()
        .faces()
        .filter_map(|face| match &face.source {
            Source::SharedFile(_, data) => Some(Arc::downgrade(data)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if maps.is_empty() {
        return Err("real lazy Tree has no SharedFile source".into());
    }
    Ok(maps)
}

fn assert_known_shared_route(snapshot: &raster_observer::Snapshot) {
    print_tree_snapshot(snapshot);
    assert_eq!(snapshot.dropped, 0);
    let selectors = snapshot
        .events
        .iter()
        .filter(|event| event["stage"] == "selector")
        .collect::<Vec<_>>();
    assert_eq!(selectors.len(), 1);
    assert_eq!(selectors[0]["route"], "known_probe");
    assert_eq!(selectors[0]["admitted"], true);
    assert_eq!(selectors[0]["actualFace"]["sourceKind"], "SharedFile");
    assert_eq!(event_count(snapshot, "route", "known_return"), 1);
    assert_eq!(event_count(snapshot, "stage", "known_parse_result"), 1);
    assert!(
        snapshot
            .events
            .iter()
            .any(|event| event["stage"] == "known_parse_result"
                && event["success"] == true
                && event["valid"] == true
                && event["selected"] == true
                && event["treeParseNs"].as_u64().is_some())
    );
    assert_eq!(event_count(snapshot, "stage", "fallback"), 0);
    assert_no_eager(snapshot);
}

fn print_tree_snapshot(snapshot: &raster_observer::Snapshot) {
    for event in &snapshot.events {
        eprintln!("tree parse observer snapshot: {event}");
    }
}

fn event_count(snapshot: &raster_observer::Snapshot, key: &str, value: &str) -> usize {
    snapshot
        .events
        .iter()
        .filter(|event| event[key] == value)
        .count()
}

fn assert_no_eager(snapshot: &raster_observer::Snapshot) {
    for event in &snapshot.events {
        for key in ["route", "stage"] {
            assert!(
                !event[key]
                    .as_str()
                    .is_some_and(|value| value.starts_with("eager"))
            );
        }
    }
}

fn assert_persistent_memo_is_file() -> Result<(), String> {
    SvgRasterizeOps::with_html_font_render_scope(assert_persistent_file_in_scope)
}

fn assert_persistent_file_in_scope() -> Result<(), String> {
    let request = HtmlFontRequest::from_text("Arial", TEXT);
    let cached = discovery_cache::known_for(&request).ok_or("known memo missing")?;
    let selected = text_shaping::known_matching(&cached.database, "Arial", NORMAL_WEIGHT, false)
        .ok_or("original File selection missing")?;
    let face = cached
        .database
        .face(selected)
        .ok_or("cached face missing")?;
    assert!(matches!(face.source, Source::File(_)));
    assert!(
        cached
            .database
            .faces()
            .all(|face| !matches!(face.source, Source::SharedFile(_, _)))
    );
    Ok(())
}

fn assert_real_render(tree: &usvg::Tree) -> Result<(), String> {
    let compatible = SvgRasterizeOps::preprocess_for_rasterizer(SVG);
    let actual =
        SvgRasterizeOps::render_tree(tree, 1.0, &compatible).map_err(|error| error.to_string())?;
    let request = HtmlFontRequest::from_text("Arial", TEXT);
    let database = Arc::new(sources::build_html_font_db(&request));
    let mut options = super::super::super::super::rasterizer_options_with_font_db(database);
    text_shaping::install_html_font_resolver(&mut options);
    let expected = SvgRasterizeOps::rasterize_with_options(SVG, 1.0, &options)
        .map_err(|error| error.to_string())?;
    assert_eq!(
        (actual.width, actual.height),
        (expected.width, expected.height)
    );
    assert_eq!(actual.rgba, expected.rgba);
    Ok(())
}

#[path = "svg_rasterize_html_font_shared_tree_scope_tests.rs"]
mod scope_tests;
