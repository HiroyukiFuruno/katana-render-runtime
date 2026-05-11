use super::ZenumlV8RenderOps;
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn renders_zenuml_svg_through_v8_runtime() {
    let result = ZenumlV8RenderOps::render(
        "zenuml\ntitle Unit Test\nA.method()",
        DiagramColorPreset::dark(),
        "unit-zenuml-v8".to_string(),
    );

    assert!(
        result
            .as_ref()
            .is_ok_and(|svg| svg.contains("<svg") && svg.contains("viewBox=")),
        "{result:?}"
    );
    assert!(
        result
            .as_ref()
            .is_ok_and(|svg| !svg.contains("<foreignObject")),
        "{result:?}"
    );
    assert!(
        result
            .as_ref()
            .is_ok_and(|svg| !svg.contains("data:image/png;base64,")),
        "{result:?}"
    );
}

#[test]
fn renders_zenuml_without_leading_keyword_line() {
    let with_keyword = ZenumlV8RenderOps::render(
        "zenuml\ntitle Strip Test\nA.method()",
        DiagramColorPreset::dark(),
        "strip-test".to_string(),
    );
    let without_keyword = ZenumlV8RenderOps::render(
        "title Strip Test\nA.method()",
        DiagramColorPreset::dark(),
        "strip-test-bare".to_string(),
    );
    assert!(
        with_keyword.as_ref().is_ok_and(|s| s.contains("<svg")),
        "{with_keyword:?}"
    );
    assert!(
        without_keyword.as_ref().is_ok_and(|s| s.contains("<svg")),
        "{without_keyword:?}"
    );
}

#[test]
fn read_asset_file_reports_missing_file_error() {
    let result = super::read_asset_file(std::path::Path::new("target/kcf-tests/missing-zenuml.js"));
    assert!(result.is_err());
    assert!(
        result
            .as_ref()
            .is_err_and(|e| e.contains("Failed to read zenuml.js:")),
        "{result:?}"
    );
}

#[test]
fn build_preamble_json_encodes_source() {
    let preamble = super::build_preamble("title \"hello\"");
    assert!(preamble.contains("var __zenuml_source__"));
    assert!(preamble.contains(r#"\"hello\""#));
}
