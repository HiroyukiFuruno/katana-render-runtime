use super::HtmlExportTemplate;
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn relative_path_resolution_handles_unclosed_src_and_absolute_urls() {
    let resolved = HtmlExportTemplate::resolve_relative_paths(
        r#"<img src="a file.png"><img src="data:image/png;base64,aaa"><img src="broken"#,
        std::path::Path::new("/tmp/doc"),
    );

    assert!(resolved.contains(r#"src="file:///tmp/doc/a%20file.png""#));
    assert!(resolved.contains(r#"src="data:image/png;base64,aaa""#));
    assert!(resolved.ends_with(r#"src="broken"#));
}

#[test]
fn generated_css_resolves_transparent_background_for_light_and_dark_presets() {
    let light_css = HtmlExportTemplate::generate_css(DiagramColorPreset::light());
    let dark_css = HtmlExportTemplate::generate_css(DiagramColorPreset::dark());
    let custom = DiagramColorPreset {
        background: "#123456".into(),
        text: "#eeeeee".into(),
        ..DiagramColorPreset::default()
    };
    let custom_css = HtmlExportTemplate::generate_css(&custom);

    assert!(light_css.contains("background-color: #ffffff"));
    assert!(dark_css.contains("background-color: #1e1e1e"));
    assert!(custom_css.contains("background-color: #123456"));
}
