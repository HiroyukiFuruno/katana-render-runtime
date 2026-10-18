use super::normalize_marker_selectors;
use crate::markdown::svg_rasterize::SvgRasterizeOps;

const RGB_CHANNEL_COUNT: usize = 3;
const RGBA_CHANNEL_COUNT: usize = 4;
const ALPHA_CHANNEL_INDEX: usize = 3;

#[test]
fn marker_selector_suffix_and_exact_attribute_render_identical_pixels() {
    let suffix = marker_svg(r#"[id$="extensionEnd"]"#);
    let exact = marker_svg(r#"[id="actual-extensionEnd"]"#);
    assert_equal_visible_pixels(&suffix, &exact, [255, 0, 0]);
}

#[test]
fn marker_selector_xml_escaped_id_suffix_matches_exact_attribute() {
    let suffix = escaped_id_svg(r#"[id$="end"]"#);
    let exact = escaped_id_svg(r#"[id="actual&amp;end"]"#);
    let normalized = normalize_marker_selectors(suffix.clone());
    assert!(
        normalized.contains(r#"[id="actual&amp;end"]"#),
        "{normalized}"
    );
    assert_equal_visible_pixels(&suffix, &exact, [255, 0, 0]);
}

#[test]
fn marker_selector_plain_exact_with_suffix_keeps_original_bytes() {
    let source = marker_svg(r#"[id$="extensionEnd"], [id="actual-extensionEnd"]"#);
    assert_eq!(normalize_marker_selectors(source.clone()), source);
}

#[test]
fn marker_selector_unsupported_escaped_exact_keeps_original_bytes() {
    for selector in [
        r#"[id$="extensionEnd"], [id="actual\22 quote"]"#,
        r#"[id$="extensionEnd"], [id="actual\5C slash"]"#,
        r#"[id$="extensionEnd"], [id="actual\A newline"]"#,
        r#"[id$="extensionEnd"], [id="one\3C two"][id="other"]"#,
    ] {
        let source = marker_svg(selector);
        assert_eq!(normalize_marker_selectors(source.clone()), source);
    }
}

#[test]
fn marker_selector_css_hex_escaped_exact_angle_id_matches_entity_pixels() {
    let escaped = angle_escaped_id_svg(r#"[id="actual\3C angle\3E end"]"#);
    let entity = angle_escaped_id_svg(r#"[id="actual&lt;angle&gt;end"]"#);
    assert_equal_visible_pixels(&escaped, &entity, [255, 0, 0]);
}

#[test]
fn marker_selector_css_hex_escaped_exact_ampersand_id_matches_entity_pixels() {
    let escaped = escaped_id_svg(r#"[id="actual\26 end"]"#);
    let entity = escaped_id_svg(r#"[id="actual&amp;end"]"#);
    assert_equal_visible_pixels(&escaped, &entity, [255, 0, 0]);
}

#[test]
fn marker_selector_css_hex_escaped_group_keeps_exact_branch_pixels() {
    let group = angle_escaped_id_svg(r#"[id$="no-suffix-hit"], [id="actual\3C angle\3E end"]"#);
    let exact = angle_escaped_id_svg(r#"[id="actual&lt;angle&gt;end"]"#);
    assert_equal_visible_pixels(&group, &exact, [255, 0, 0]);
}

#[test]
fn marker_selector_css_hex_escaped_wrong_exact_id_stays_unmatched() {
    let wrong = angle_escaped_id_svg(r#"[id$="no-suffix-hit"], [id="actual\3C missing\3E end"]"#);
    let control = angle_escaped_id_svg(".never");
    assert_equal_visible_pixels(&wrong, &control, [255, 255, 255]);
}

#[test]
fn marker_selector_xml_escaped_angle_id_suffix_matches_exact_attribute_pixels() {
    let suffix = angle_escaped_id_svg(r#"[id$="\3C angle>end"]"#);
    let exact = angle_escaped_id_svg(r#"[id="actual&lt;angle&gt;end"]"#);
    let normalized = normalize_marker_selectors(suffix.clone());
    assert!(
        normalized.contains("actual&lt;angle&gt;end"),
        "{normalized}"
    );
    assert_equal_visible_pixels(&suffix, &exact, [255, 0, 0]);
}

#[test]
fn marker_selector_dollar_outside_style_keeps_fastpath_and_public_pixels() {
    let source = String::from(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="16"><rect id="price$end" width="24" height="16" fill="#ff0000"/></svg>"##,
    );
    let pointer = source.as_ptr();
    let capacity = source.capacity();
    let normalized = normalize_marker_selectors(source);
    assert_eq!(
        normalized,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="16"><rect id="price$end" width="24" height="16" fill="#ff0000"/></svg>"##
    );
    assert_eq!(normalized.as_ptr(), pointer);
    assert_eq!(normalized.capacity(), capacity);
    assert_public_raster(&normalized, 24, 16, [255, 0, 0]);
}

#[test]
fn marker_selector_zero_and_empty_matches_keep_sibling_branch_pixels() {
    let class_only = rect_svg(".extension { fill: #ff0000 !important; }");
    let zero_hit = rect_svg(r#"[id$="missing"], .extension { fill: #ff0000 !important; }"#);
    let empty = rect_svg(r#"[id$=""], .extension { fill: #ff0000 !important; }"#);
    assert_equal_visible_pixels(&class_only, &zero_hit, [255, 0, 0]);
    assert_equal_visible_pixels(&class_only, &empty, [255, 0, 0]);
}

#[test]
fn marker_selector_suffix_preserves_attribute_specificity() {
    let suffix = rect_svg(r#"[id$="end"] { fill: #ff0000; } .extension { fill: #0000ff; }"#);
    let exact = rect_svg(r#"[id="actual-end"] { fill: #ff0000; } .extension { fill: #0000ff; }"#);
    assert_equal_visible_pixels(&suffix, &exact, [0, 0, 255]);
}

#[test]
fn marker_selector_unsupported_preludes_remain_byte_identical() {
    for prelude in [
        r#"[id$="end" i]"#,
        r#":is([id$="end"]), .extension"#,
        r#":is(.a), [id$="end"]"#,
        r#"[id$="foo"][id$="bar"]"#,
        r#"[ns|id$="end"]"#,
        r#"[id$=end"#,
    ] {
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><style>{prelude} {{ fill: red; }}</style></svg>"#
        );
        assert_eq!(normalize_marker_selectors(source.clone()), source);
    }
}

#[test]
fn marker_selector_candidate_free_svg_keeps_owned_string_allocation() {
    let source = String::from(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.extension { fill: red; }</style></svg>"#,
    );
    let pointer = source.as_ptr();
    let capacity = source.capacity();
    let normalized = normalize_marker_selectors(source);
    assert_eq!(normalized.as_ptr(), pointer);
    assert_eq!(normalized.capacity(), capacity);
}

#[test]
fn marker_selector_entity_and_cdata_styles_remain_byte_identical() {
    let entity = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>[id$="en&#100;"] { fill: red; }</style></svg>"#;
    let cdata = r#"<svg xmlns="http://www.w3.org/2000/svg"><style><![CDATA[[id$="end"] { fill: red; }]]></style></svg>"#;
    assert_eq!(normalize_marker_selectors(entity.to_owned()), entity);
    assert_eq!(normalize_marker_selectors(cdata.to_owned()), cdata);
}

#[test]
fn marker_selector_only_rewrites_svg_or_unqualified_style_elements() {
    let unqualified = r#"<svg id="root"><style>#root [id$="end"] { fill: red; }</style><rect id="actual-end"/></svg>"#;
    let foreign = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:html="http://www.w3.org/1999/xhtml"><html:style>[id$="end"] { fill: red; }</html:style><rect id="actual-end"/></svg>"#;
    let rewritten = normalize_marker_selectors(unqualified.to_owned());
    assert!(rewritten.contains(r#"[id="actual-end"]"#));
    assert_eq!(normalize_marker_selectors(foreign.to_owned()), foreign);
}

fn marker_svg(selector: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" id="root" width="96" height="64" viewBox="0 0 96 64"><defs><marker id="arrow" markerWidth="12" markerHeight="12" refX="10" refY="0" orient="auto" markerUnits="userSpaceOnUse"><path id="actual-extensionEnd" class="extension" d="M0,-5 L10,0 L0,5 Z"/></marker></defs><style>#root {selector}, #root .extension {{ fill: #ff0000 !important; stroke: #ff0000 !important; stroke-width: 1px; }}</style><path id="root-path" class="connector" d="M12 32 H60" fill="none" stroke="#333333" stroke-width="2" marker-end="url(#arrow)"/></svg>"##
    )
}

fn escaped_id_svg(selector: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="16"><style>{selector} {{ fill: #ff0000; }}</style><rect id="actual&amp;end" width="24" height="16" fill="#ffffff"/></svg>"##
    )
}

fn angle_escaped_id_svg(selector: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="16"><style>{selector} {{ fill: #ff0000; }}</style><rect id="actual&lt;angle&gt;end" width="24" height="16" fill="#ffffff"/></svg>"##
    )
}

fn assert_public_raster(
    svg: &str,
    expected_width: u32,
    expected_height: u32,
    visible_color: [u8; RGB_CHANNEL_COUNT],
) {
    assert!(
        rendered_pixels(svg).is_some_and(|(width, height, rgba)| {
            width == expected_width && height == expected_height && has_color(&rgba, visible_color)
        }),
        "expected {expected_width}x{expected_height} raster with visible {visible_color:?}: {svg}"
    );
}

fn rect_svg(style: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="16"><style>{style}</style><rect id="actual-end" class="extension" width="10" height="16" fill="#ffffff"/><rect id="other-blue" x="12" width="10" height="16" fill="#0000ff"/></svg>"##
    )
}

fn assert_equal_visible_pixels(
    left_svg: &str,
    right_svg: &str,
    visible_color: [u8; RGB_CHANNEL_COUNT],
) {
    let left_normalized = SvgRasterizeOps::preprocess_for_rasterizer(left_svg);
    let right_normalized = SvgRasterizeOps::preprocess_for_rasterizer(right_svg);
    let left_pixels = rendered_pixels(left_svg);
    let right_pixels = rendered_pixels(right_svg);
    let diagnostic = format!(
        "left test SVG:\n{left_svg}\nleft normalized SVG:\n{left_normalized}\nright test SVG:\n{right_svg}\nright normalized SVG:\n{right_normalized}"
    );
    assert_same_dimensions(&left_pixels, &right_pixels, &diagnostic);
    assert_same_rgba(&left_pixels, &right_pixels, &diagnostic);
    assert_visible_color(&left_pixels, visible_color, "left", &diagnostic);
    assert_visible_color(&right_pixels, visible_color, "right", &diagnostic);
}

fn assert_same_dimensions(
    left: &Option<(u32, u32, Vec<u8>)>,
    right: &Option<(u32, u32, Vec<u8>)>,
    diagnostic: &str,
) {
    assert!(
        left.as_ref()
            .zip(right.as_ref())
            .is_some_and(|(left, right)| left.0 == right.0 && left.1 == right.1),
        "raster dimensions differ or rasterization failed; {diagnostic}"
    );
}

fn assert_same_rgba(
    left: &Option<(u32, u32, Vec<u8>)>,
    right: &Option<(u32, u32, Vec<u8>)>,
    diagnostic: &str,
) {
    assert!(
        left.as_ref()
            .zip(right.as_ref())
            .is_some_and(|(left, right)| left.2 == right.2),
        "RGBA buffers differ or rasterization failed; {diagnostic}"
    );
}

fn assert_visible_color(
    pixels: &Option<(u32, u32, Vec<u8>)>,
    color: [u8; RGB_CHANNEL_COUNT],
    side: &str,
    diagnostic: &str,
) {
    assert!(
        pixels
            .as_ref()
            .is_some_and(|(_, _, rgba)| has_color(rgba, color)),
        "expected visible {color:?} color on {side} raster; {diagnostic}"
    );
}

fn rendered_pixels(svg: &str) -> Option<(u32, u32, Vec<u8>)> {
    SvgRasterizeOps::rasterize_svg(svg, 1.0)
        .ok()
        .map(|image| (image.width, image.height, image.rgba))
}

fn has_color(pixels: &[u8], color: [u8; RGB_CHANNEL_COUNT]) -> bool {
    pixels.chunks_exact(RGBA_CHANNEL_COUNT).any(|pixel| {
        pixel
            .get(..RGB_CHANNEL_COUNT)
            .is_some_and(|channels| channels == color)
            && pixel
                .get(ALPHA_CHANNEL_INDEX)
                .is_some_and(|alpha| *alpha > 0)
    })
}

#[path = "svg_rasterize_preprocess_marker_exact_public_tests.rs"]
mod exact_public_tests;
