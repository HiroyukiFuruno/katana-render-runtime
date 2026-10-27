use super::{decoded_ascii, parse_with_document, rasterize};
use crate::markdown::svg_rasterize::{
    GeneratedClipProvenance, GeneratedHtmlTextProvenance, RasterizedSvg, SvgRasterizeError,
};
use resvg::usvg;

const DTD_SVG: &str = r#"<!DOCTYPE svg><svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="red"/></svg>"#;

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
