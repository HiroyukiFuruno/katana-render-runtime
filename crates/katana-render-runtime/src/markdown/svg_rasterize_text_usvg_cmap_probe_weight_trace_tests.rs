use super::{Records, selectors, spans};
use crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;
use std::sync::Arc;

const FALLBACK_TRACE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="32"><text id="weight-runs" x="0" y="20" font-family="Noto Sans">Aあ</text></svg>"#;

#[test]
fn layout_trace_rejects_a_non_text_target_in_a_valid_svg() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="32"><rect id="weight-runs" width="20" height="16"/></svg>"#;
    let options = resvg::usvg::Options::default();
    let records = || Arc::new(std::sync::Mutex::new(Vec::new()));

    assert_eq!(
        spans::capture(svg, &options, records(), records()),
        Err("mixed-weight SVG text node missing".into())
    );
}

#[test]
fn layout_trace_propagates_the_svg_parse_error() -> Result<(), String> {
    let svg = "<svg xmlns='http://www.w3.org/2000/svg'><text>";
    let options = resvg::usvg::Options::default();
    let expected = resvg::usvg::Tree::from_str(svg, &options)
        .err()
        .ok_or("malformed fixture must fail SVG parsing")?;
    assert_eq!(
        spans::capture(svg, &options, Records::default(), Records::default()),
        Err(expected.to_string())
    );
    Ok(())
}

#[test]
fn layout_trace_reports_a_failed_record_read_for_either_selector() -> Result<(), String> {
    let (options, selected, fallback) = fallback_trace_fixture();
    let capture = || {
        spans::capture(
            FALLBACK_TRACE_SVG,
            &options,
            Arc::clone(&selected),
            Arc::clone(&fallback),
        )
    };
    let healthy = capture()?;
    assert!(
        healthy["fallbackSelections"]
            .as_array()
            .is_some_and(|rows| !rows.is_empty())
    );
    for records in [&fallback, &selected] {
        poison_trace_records(records)?;
        assert_eq!(capture(), Err("font trace lock poisoned".into()));
    }
    Ok(())
}

#[test]
fn poisoned_trace_records_do_not_change_real_font_selection_or_fallback() -> Result<(), String> {
    let (options, selected, fallback) = fallback_trace_fixture();
    let control = parse_fallback_trace(&options)?;
    for records in [&selected, &fallback] {
        assert!(
            !records
                .lock()
                .map_err(|error| error.to_string())?
                .is_empty()
        );
        poison_trace_records(records)?;
    }
    let candidate = parse_fallback_trace(&options)?;
    let write = resvg::usvg::WriteOptions::default();
    assert_eq!(candidate.to_string(&write), control.to_string(&write));
    Ok(())
}

fn fallback_trace_fixture() -> (resvg::usvg::Options<'static>, Records, Records) {
    let mut options = resvg::usvg::Options {
        fontdb: Arc::new(fallback_trace_database()),
        ..resvg::usvg::Options::default()
    };
    let selected = Records::default();
    let fallback = Records::default();
    selectors::install(&mut options, Arc::clone(&selected), Arc::clone(&fallback));
    (options, selected, fallback)
}

fn fallback_trace_database() -> resvg::usvg::fontdb::Database {
    const CJK: &[u8] =
        include_bytes!("../../../../tests/fixtures/fonts/han-kerning/NotoSansCJKjp-han14.otf");
    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
    database.load_font_data(CJK.to_vec());
    database
}

fn parse_fallback_trace(
    options: &resvg::usvg::Options<'static>,
) -> Result<resvg::usvg::Tree, String> {
    let tree = crate::markdown::svg_rasterize::text_shaping::with_validated_tree_parse(|| {
        resvg::usvg::Tree::from_str(FALLBACK_TRACE_SVG, options)
    })
    .map_err(|error| error.to_string())?;
    let Some(resvg::usvg::Node::Text(text)) = tree.node_by_id("weight-runs") else {
        return Err("fallback trace text missing".into());
    };
    let faces = text
        .layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs)
        .map(|glyph| glyph.font)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(faces.len(), 2);
    Ok(tree)
}

fn poison_trace_records(records: &Records) -> Result<(), String> {
    let worker_records = Arc::clone(records);
    let result = std::thread::spawn(move || -> Result<(), String> {
        let _guard = worker_records.lock().map_err(|error| error.to_string())?;
        std::panic::resume_unwind(Box::new("trace recorder interrupted"))
    })
    .join();
    assert!(result.is_err());
    assert!(records.is_poisoned());
    Ok(())
}

#[test]
fn layout_trace_preserves_binary_and_shared_font_sources() -> Result<(), String> {
    use resvg::usvg::fontdb::Source;

    let bytes = FONT_BYTES;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/NotoSans-Regular.ttf");
    let expected = "2ec33f84606cbaa0a1a944488e14f97faf2f6a25ecdd8354f5358f06da13c7d9";
    for (source, kind) in [
        (Source::Binary(Arc::new(bytes.to_vec())), "Binary"),
        (
            Source::SharedFile(path, Arc::new(bytes.to_vec())),
            "SharedFile",
        ),
    ] {
        assert_layout_source(source, kind, expected)?;
    }
    Ok(())
}

fn assert_layout_source(
    source: resvg::usvg::fontdb::Source,
    kind: &str,
    expected: &str,
) -> Result<(), String> {
    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_font_source(source);
    let options = resvg::usvg::Options {
        fontdb: Arc::new(database),
        ..resvg::usvg::Options::default()
    };
    let records = || Arc::new(std::sync::Mutex::new(Vec::new()));
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="32"><text id="weight-runs" x="0" y="20" font-family="Noto Sans">AV</text></svg>"#;
    let trace = spans::capture(svg, &options, records(), records())?;
    let spans = trace["spans"].as_array().ok_or("layout spans missing")?;
    assert!(!spans.is_empty());
    for span in spans {
        assert!(span["glyph_count"].as_u64().is_some_and(|count| count > 0));
        assert_span_faces(span, kind, expected)?;
    }
    Ok(())
}

fn assert_span_faces(span: &serde_json::Value, kind: &str, expected: &str) -> Result<(), String> {
    let faces = span["faces"].as_array().ok_or("layout faces missing")?;
    assert!(!faces.is_empty());
    for face in faces {
        assert_eq!(face["source"]["kind"], kind);
        assert_eq!(face["data"]["sha256"], expected);
        assert_eq!(face["data"]["index"], 0);
        assert_eq!(face["face_index"], 0);
    }
    Ok(())
}
