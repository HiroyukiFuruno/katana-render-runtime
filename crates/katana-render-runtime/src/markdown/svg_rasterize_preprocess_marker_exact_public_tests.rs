use super::{
    RGBA_CHANNEL_COUNT, angle_escaped_id_svg, assert_equal_visible_pixels, escaped_id_svg,
    normalize_marker_selectors, rect_svg, rendered_pixels,
};

const RED: [u8; 3] = [255, 0, 0];

#[test]
fn marker_selector_escaped_non_id_preserves_source_bytes() {
    let escaped = rect_svg(r#"[cl\61 ss="extension"] { fill: #ff0000 !important; }"#);
    assert_eq!(normalize_marker_selectors(escaped.clone()), escaped);
}

#[test]
fn marker_selector_unsupported_escaped_exact_forms_preserve_source_bytes() {
    for selector in [
        r#"[ id = "actual\00003C angle\00003Eend" ]"#,
        r#"[id=actual\00003Cangle\00003Eend]"#,
    ] {
        let source = angle_escaped_id_svg(selector);
        assert_eq!(normalize_marker_selectors(source.clone()), source);
    }
}

#[test]
fn marker_selector_six_digit_css_escape_matches_entity_pixels() {
    let escaped = escaped_id_svg(r#"[id="actual\000026 end"]"#);
    let entity = escaped_id_svg(r#"[id="actual&amp;end"]"#);
    let normalized = normalize_marker_selectors(escaped.clone());
    assert!(
        normalized.contains(r#"[id="actual&amp;end"]"#),
        "{normalized}"
    );
    assert_equal_visible_pixels(&escaped, &entity, RED);
    assert!(
        rendered_pixels(&escaped).is_some_and(|(width, height, rgba)| {
            width == 24
                && height == 16
                && rgba
                    .chunks_exact(RGBA_CHANNEL_COUNT)
                    .any(|pixel| pixel[..3] == RED && pixel[3] == u8::MAX)
        }),
        "expected visible opaque red pixels"
    );
}
