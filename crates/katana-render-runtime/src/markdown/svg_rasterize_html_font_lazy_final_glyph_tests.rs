use crate::markdown::svg_rasterize::SvgRasterizeOps;

#[test]
fn actual_raster_route_retains_each_top_and_embedded_svg_tree() -> Result<(), String> {
    let (first, second) = svg_inputs();
    let (renders, audit) = SvgRasterizeOps::with_actual_font_source_audit(|| {
        SvgRasterizeOps::rasterize_html_svg(&first, 1.0)?;
        SvgRasterizeOps::rasterize_html_svg(&second, 1.0)
    })
    .map_err(|error| error.to_string())?;
    renders.map_err(|error| error.to_string())?;
    assert_svg_tree_rows(&audit, &first, &second)?;
    assert_eq!(audit["finalGlyphDropped"], 0);
    Ok(())
}

fn svg_inputs() -> (String, String) {
    use base64::Engine;

    let embedded = r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="24"><text x="1" y="18" font-family="Arial" font-size="16">nested</text></svg>"#;
    let encoded = base64::engine::general_purpose::STANDARD.encode(embedded);
    let first = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="28"><text x="1" y="20" font-family="Arial" font-size="16">first</text></svg>"#.to_owned();
    let second = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="32"><text x="1" y="20" font-family="Arial" font-size="16">second</text><image x="30" y="1" width="64" height="24" href="data:image/svg+xml;base64,{encoded}"/></svg>"#
    );
    (first, second)
}

fn assert_svg_tree_rows(
    audit: &serde_json::Value,
    first: &str,
    second: &str,
) -> Result<(), String> {
    let trees = audit["finalGlyphTrees"]
        .as_array()
        .ok_or_else(|| "final glyph tree evidence is absent".to_owned())?;
    assert_eq!(audit["finalGlyphPaintCalls"], 2);
    for (call, source) in [(1, first), (2, second)] {
        assert_tree_row(
            trees,
            call,
            "root",
            super::output::sha256(source.as_bytes()),
        );
    }
    assert!(trees.iter().any(|tree| {
        tree["paintCall"] == 2
            && tree["treePath"]
                .as_str()
                .is_some_and(|path| path.contains("image-svg@root/"))
            && tree["sourceSvg"].is_null()
            && tree["resolvedSvg"].is_null()
            && tree["resolvedTextSvg"].is_null()
            && tree["resolvedTextSvgSha256"].is_null()
    }));
    Ok(())
}

fn assert_tree_row(trees: &[serde_json::Value], call: u64, path: &str, sha: String) {
    assert!(trees.iter().any(|tree| {
        tree["paintCall"] == call
            && tree["treePath"] == path
            && tree["sourceSvgSha256"] == sha
            && tree["sourceSvg"]
                .as_str()
                .is_some_and(|source| super::output::sha256(source.as_bytes()) == sha)
            && tree["resolvedSvg"].is_string()
            && tree["resolvedSvg"].as_str().is_some_and(|resolved| {
                tree["resolvedSvgSha256"] == super::output::sha256(resolved.as_bytes())
            })
            && tree["resolvedTextSvg"].is_string()
            && tree["resolvedTextSvg"].as_str().is_some_and(|resolved| {
                tree["resolvedTextSvgSha256"] == super::output::sha256(resolved.as_bytes())
                    && resolved.contains("<text")
            })
            && tree["resolvedSvg"]
                .as_str()
                .is_some_and(|resolved| resolved.contains("<path"))
    }));
}
