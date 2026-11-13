use super::{decoded_ascii, parse_with_document, rasterize};
use crate::markdown::svg_rasterize::{
    GeneratedClipProvenance, GeneratedHtmlTextProvenance, RasterizedSvg, SvgRasterizeError,
};
use resvg::usvg;

const DTD_SVG: &str = r#"<!DOCTYPE svg><svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="red"/></svg>"#;
const DIAGNOSTIC_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="32"><text x="2" y="24" font-family="Noto Sans">ABC</text></svg>"#;

#[test]
fn stale_diagnostic_font_falls_back_to_the_normal_svg_tree()
-> Result<(), Box<dyn std::error::Error>> {
    use super::super::raster_observer;

    let stale = stale_diagnostic_database()?;
    let document = usvg::roxmltree::Document::parse(DIAGNOSTIC_SVG);
    let text = decoded_ascii(&document).ok_or("expected ASCII fixture")?;
    let expected = super::eager(DIAGNOSTIC_SVG, DIAGNOSTIC_SVG)?;
    let observer = raster_observer::Scope::enter();
    let options = super::diagnostic::prepare_file_options(stale, text);
    let tree = super::parse_with_options(DIAGNOSTIC_SVG, DIAGNOSTIC_SVG, options)?;
    let output = usvg::WriteOptions::default();
    assert_eq!(tree.to_string(&output), expected.to_string(&output));
    assert_stale_diagnostic_fallback(&observer.snapshot());
    Ok(())
}

fn stale_diagnostic_database()
-> Result<Option<super::super::discovery::KnownCacheValue>, Box<dyn std::error::Error>> {
    use super::super::{lazy_diagnostic, proof::CreationProof};
    use std::sync::Arc;

    let directory = tempfile::tempdir()?;
    let font_path = directory.path().join("font.ttf");
    std::fs::write(
        &font_path,
        crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES,
    )?;
    let mut database = usvg::fontdb::Database::new();
    let mut proof = CreationProof::default();
    proof.load(
        &mut database,
        &[font_path.to_str().ok_or("invalid font path")?],
    );
    proof.capture_prefix(&database);
    assert!(!database.is_empty() && proof.current(&database));
    std::fs::remove_file(font_path)?;
    let result =
        lazy_diagnostic::validate_file_fixture_database(Arc::new(database), Arc::new(proof));
    assert!(result.is_none());
    Ok(result)
}

fn assert_stale_diagnostic_fallback(snapshot: &super::super::raster_observer::Snapshot) {
    let events = &snapshot.events;
    assert!(
        events
            .iter()
            .any(|event| event["reason"] == "known_for_unavailable")
    );
    assert!(
        events
            .iter()
            .any(|event| event["route"] == "eager_prepare_unavailable")
    );
    assert!(
        events
            .iter()
            .all(|event| event["stage"] != "known_parse_attempt")
    );
}

fn registered_provenance() -> GeneratedHtmlTextProvenance {
    let mut provenance = GeneratedHtmlTextProvenance::default();
    let _ = provenance.record(r#"<text x="1" y="2">Label</text>"#.to_string());
    provenance
}

fn shared_render(
    svg: &str,
    provenance: &GeneratedHtmlTextProvenance,
) -> Result<RasterizedSvg, SvgRasterizeError> {
    rasterize(svg, 1.0, &GeneratedClipProvenance::default(), provenance)
}

#[test]
fn parse_failed_preserves_external_and_owned_error_messages() -> Result<(), String> {
    let error = usvg::roxmltree::Document::parse("<svg")
        .err()
        .ok_or("invalid XML unexpectedly parsed")?;
    let expected = error.to_string();
    for actual in [
        SvgRasterizeError::parse_failed(error),
        SvgRasterizeError::parse_failed(expected.clone()),
    ] {
        assert_parse_error::<()>(Err(actual), &expected)?;
    }
    Ok(())
}

#[test]
fn shared_xml_ascii_preparation_preserves_decoded_text() {
    let cases = [
        ("A&amp;B<![CDATA[>C]]>", Some("A&B>C")),
        ("&#x65E5;", None),
        ("<![CDATA[語]]>", None),
        ("", None),
    ];
    for (text, expected) in cases {
        let svg = format!(r#"<svg><text>{text}</text></svg>"#);
        let document = usvg::roxmltree::Document::parse(&svg);
        assert_eq!(decoded_ascii(&document).as_deref(), expected);
    }
}

#[test]
fn shared_xml_dtd_empty_provenance_keeps_eager_rendering() -> Result<(), String> {
    let document = usvg::roxmltree::Document::parse(DTD_SVG);
    assert!(document.is_err());
    assert!(decoded_ascii(&document).is_none());
    let tree =
        parse_with_document(DTD_SVG, DTD_SVG, &document).map_err(|error| error.to_string())?;
    assert_eq!(tree.size().width(), 8.0);
    let provenance = GeneratedHtmlTextProvenance::default();
    let routing = provenance
        .validated_routing_with_document(&document, &tree)
        .map_err(|error| error.to_string())?;
    assert!(!routing.all_text_verified);
    let rendered = shared_render(DTD_SVG, &provenance).map_err(|error| error.to_string())?;
    assert!(rendered.rgba.chunks_exact(4).any(|pixel| pixel[3] != 0));
    Ok(())
}

#[test]
fn shared_xml_dtd_registered_provenance_rejects_after_eager_parse() -> Result<(), String> {
    let document = usvg::roxmltree::Document::parse(DTD_SVG);
    let tree =
        parse_with_document(DTD_SVG, DTD_SVG, &document).map_err(|error| error.to_string())?;
    let error = document.as_ref().err().ok_or("DTD unexpectedly parsed")?;
    let expected = error.to_string();
    let provenance = registered_provenance();
    assert_parse_error(
        provenance.validated_routing_with_document(&document, &tree),
        &expected,
    )?;
    assert_parse_error(shared_render(DTD_SVG, &provenance), &expected)
}

#[test]
fn shared_xml_invalid_source_keeps_usvg_error_before_routing() -> Result<(), String> {
    let svg = "<svg";
    let expected = super::eager(svg, svg)
        .err()
        .ok_or("invalid source unexpectedly parsed")?
        .to_string();
    assert_parse_error(shared_render(svg, &registered_provenance()), &expected)
}

#[test]
fn shared_xml_invalid_size_propagates_known_parse_error_without_eager_retry() -> Result<(), String>
{
    use super::super::{lazy_diagnostic, raster_observer};

    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="0" height="0"><text font-family="Noto Sans">ABC</text></svg>"#;
    let document = usvg::roxmltree::Document::parse(svg);
    assert!(document.is_ok());
    assert_eq!(decoded_ascii(&document).as_deref(), Some("ABC"));
    let expected = super::eager(svg, svg)
        .err()
        .ok_or("zero-sized SVG unexpectedly parsed")?
        .to_string();
    let scope = raster_observer::Scope::enter();
    let (actual, capture) = lazy_diagnostic::with_route(
        lazy_diagnostic::SourceRoute::File,
        lazy_diagnostic::Mode::Audit,
        || parse_with_document(svg, svg, &document),
    );
    assert_eq!(
        actual
            .err()
            .ok_or("known parse unexpectedly succeeded")?
            .to_string(),
        expected
    );
    assert!(capture.errors.is_empty());
    assert_known_parse_failed_without_eager_retry(&scope.snapshot());
    Ok(())
}

fn assert_known_parse_failed_without_eager_retry(
    snapshot: &super::super::raster_observer::Snapshot,
) {
    assert!(snapshot.complete());
    assert!(
        snapshot
            .events
            .iter()
            .any(|event| event["stage"] == "prepare" && event["success"] == true)
    );
    assert!(
        snapshot
            .events
            .iter()
            .any(|event| { event["stage"] == "known_parse_result" && event["success"] == false })
    );
    assert!(
        !snapshot
            .events
            .iter()
            .any(|event| event["stage"] == "eager_parse_attempt")
    );
}

fn assert_parse_error<T>(
    actual: Result<T, SvgRasterizeError>,
    expected: &str,
) -> Result<(), String> {
    match actual {
        Err(SvgRasterizeError::ParseFailed(reason)) => {
            assert_eq!(reason, expected);
            Ok(())
        }
        _ => Err("expected unchanged XML parse error".to_string()),
    }
}
