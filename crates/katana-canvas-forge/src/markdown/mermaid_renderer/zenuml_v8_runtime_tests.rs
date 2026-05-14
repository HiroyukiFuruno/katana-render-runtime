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
fn render_script_json_encodes_source() {
    let script = super::render_script("title \"hello\"", false);
    assert_eq!(
        script,
        r#"katanaRunZenumlRuntime("title \"hello\"", false);"#
    );
}

#[test]
fn render_script_sets_dark_true() {
    let script = super::render_script("A.method()", true);
    assert_eq!(script, r#"katanaRunZenumlRuntime("A.method()", true);"#);
}

#[test]
fn render_script_sets_dark_false() {
    let script = super::render_script("A.method()", false);
    assert_eq!(script, r#"katanaRunZenumlRuntime("A.method()", false);"#);
}

#[test]
fn renders_zenuml_svg_in_light_mode() {
    let mut preset = DiagramColorPreset::dark().clone();
    preset.dark_mode = false;
    let result = ZenumlV8RenderOps::render(
        "zenuml\ntitle Light Test\nA.method()",
        &preset,
        "light-test".to_string(),
    );
    assert!(
        result.as_ref().is_ok_and(|svg| svg.contains("<svg")),
        "{result:?}"
    );
}

#[test]
fn dark_mode_injects_style_block_with_css_variables() {
    let result = ZenumlV8RenderOps::render(
        "zenuml\ntitle Dark Test\nA.method()",
        DiagramColorPreset::dark(),
        "dark-style-test".to_string(),
    );
    assert!(
        result.as_ref().is_ok_and(|svg| {
            svg.contains("<style>") && svg.contains(".participant-box{fill:#5964f2")
        }),
        "{result:?}"
    );
    /* WHY: dark style block must appear after </defs> to win the cascade over WZ stylesheet */
    assert!(
        result.as_ref().is_ok_and(|svg| {
            let defs_end = svg.find("</defs>").unwrap_or(0);
            let style_pos = svg.rfind("<style>").unwrap_or(0);
            style_pos > defs_end
        }),
        "dark style block should come after </defs>"
    );
}

#[test]
fn light_mode_does_not_inject_style_block() {
    let mut preset = DiagramColorPreset::dark().clone();
    preset.dark_mode = false;
    let result = ZenumlV8RenderOps::render(
        "zenuml\ntitle No-Style Test\nA.method()",
        &preset,
        "no-style-test".to_string(),
    );
    assert!(
        result
            .as_ref()
            .is_ok_and(|svg| !svg.contains(".participant-box{fill:#5964f2")),
        "{result:?}"
    );
}
