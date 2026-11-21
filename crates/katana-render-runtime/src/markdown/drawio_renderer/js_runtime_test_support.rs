#[path = "js_runtime_bundle_fixture_data.rs"]
mod fixture_data;

use fixture_data::{
    FAKE_BUNDLE_BODY, FAKE_BUNDLE_EDITOR_BODY, FAKE_BUNDLE_EDITOR_PREFIX, FAKE_BUNDLE_FOOTER,
    FAKE_BUNDLE_HEADER_HEIGHT, FAKE_BUNDLE_HEADER_MIDDLE, FAKE_BUNDLE_HEADER_PREFIX,
    FAKE_BUNDLE_VAR_EDITOR_PREFIX, FAKE_BUNDLE_VIEWBOX_PREFIX, FAKE_BUNDLE_VIEWBOX_SUFFIX,
    FAKE_BUNDLE_WITH_HTML_COMMENT_LABEL_BODY, FAKE_BUNDLE_WITH_LIGHT_DARK_SVG_PAINT_BODY,
    FAKE_SHADOW_BODY,
};

pub(super) const OFFICIAL_REFERENCE_VIEWPORT_BUNDLE_HOOK: &str =
    fixture_data::OFFICIAL_REFERENCE_VIEWPORT_BUNDLE_HOOK;
pub(super) const HTML_WRAP_BUNDLE_HOOK: &str = fixture_data::HTML_WRAP_BUNDLE_HOOK;
pub(super) const FAKE_BUNDLE_WITH_FOREIGN_OBJECT: &str =
    fixture_data::FAKE_BUNDLE_WITH_FOREIGN_OBJECT;

pub(super) fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

pub(super) fn assert_render_contains(rendered: &Result<String, String>, marker: &str) {
    assert!(
        rendered.as_ref().is_ok_and(|svg| svg.contains(marker)),
        "{rendered:?}"
    );
}

pub(super) fn assert_render_not_contains(rendered: &Result<String, String>, marker: &str) {
    assert!(
        rendered.as_ref().is_ok_and(|svg| !svg.contains(marker)),
        "{rendered:?}"
    );
}

pub(super) fn fake_bundle() -> &'static str {
    static BUNDLE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    BUNDLE
        .get_or_init(|| fake_html_label_bundle("20", "10", FAKE_BUNDLE_BODY, ""))
        .as_str()
}

pub(super) fn fake_html_label_bundle(
    width: &str,
    height: &str,
    svg_body: &str,
    after_callback: &str,
) -> String {
    fake_html_bundle(
        (width, width),
        (height, height),
        "",
        FAKE_BUNDLE_EDITOR_PREFIX,
        FAKE_BUNDLE_EDITOR_BODY,
        svg_body,
        after_callback,
    )
}

pub(super) fn fake_color_bundle(mx_utils: &str, svg_body: &str) -> String {
    fake_html_bundle(
        ("80", "80"),
        ("30", "30"),
        mx_utils,
        FAKE_BUNDLE_EDITOR_PREFIX,
        FAKE_BUNDLE_EDITOR_BODY,
        svg_body,
        "",
    )
}

pub(super) fn fake_html_text_color_bundle(mx_utils: &str, svg_body: &str) -> String {
    fake_html_bundle(
        ("80", "80"),
        ("30", "30"),
        mx_utils,
        FAKE_BUNDLE_VAR_EDITOR_PREFIX,
        FAKE_BUNDLE_EDITOR_BODY,
        svg_body,
        "",
    )
}

pub(super) fn fake_color_shape_group(fill: &str) -> String {
    [
        r#"  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "cell");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", "0");
  rect.setAttribute("y", "0");
  rect.setAttribute("width", "80");
  rect.setAttribute("height", "30");
  rect.setAttribute("fill", ""#,
        fill,
        r#"");
  group.appendChild(rect);
  svg.appendChild(group);
"#,
    ]
    .concat()
}

pub(super) fn fake_pixel_svg_bundle(
    width: &str,
    height: &str,
    svg_body: &str,
    after_callback: &str,
) -> String {
    let pixel_width = format!("{width}px");
    let pixel_height = format!("{height}px");
    fake_html_bundle(
        (&pixel_width, width),
        (&pixel_height, height),
        "",
        FAKE_BUNDLE_EDITOR_PREFIX,
        FAKE_BUNDLE_EDITOR_BODY,
        svg_body,
        after_callback,
    )
}

fn fake_html_bundle(
    (width, viewbox_width): (&str, &str),
    (height, viewbox_height): (&str, &str),
    mx_utils: &str,
    editor_prefix: &str,
    editor_body: &str,
    svg_body: &str,
    after_callback: &str,
) -> String {
    [
        FAKE_BUNDLE_HEADER_PREFIX,
        mx_utils,
        editor_prefix,
        editor_body,
        FAKE_BUNDLE_HEADER_MIDDLE,
        width,
        FAKE_BUNDLE_HEADER_HEIGHT,
        height,
        FAKE_BUNDLE_VIEWBOX_PREFIX,
        "0 0 ",
        viewbox_width,
        " ",
        viewbox_height,
        FAKE_BUNDLE_VIEWBOX_SUFFIX,
        svg_body,
        FAKE_BUNDLE_FOOTER,
        after_callback,
    ]
    .concat()
}

pub(super) fn fake_shadow_bundle() -> String {
    fake_html_label_bundle("20", "10", FAKE_SHADOW_BODY, "")
}

pub(super) fn fake_bundle_with_foreign_object() -> &'static str {
    FAKE_BUNDLE_WITH_FOREIGN_OBJECT
}

pub(super) fn fake_bundle_with_html_comment_label() -> &'static str {
    static BUNDLE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    BUNDLE
        .get_or_init(|| {
            fake_html_bundle(
                ("20", "20"),
                ("10", "10"),
                "",
                FAKE_BUNDLE_EDITOR_PREFIX,
                "    return katanaDrawioHtmlLabelText(value);\n",
                FAKE_BUNDLE_WITH_HTML_COMMENT_LABEL_BODY,
                "",
            )
        })
        .as_str()
}

pub(super) fn fake_bundle_with_light_dark_svg_paint() -> &'static str {
    static BUNDLE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    BUNDLE
        .get_or_init(|| {
            fake_html_label_bundle("20", "10", FAKE_BUNDLE_WITH_LIGHT_DARK_SVG_PAINT_BODY, "")
        })
        .as_str()
}
