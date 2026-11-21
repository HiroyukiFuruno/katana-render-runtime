use super::CssStyle;

const EPSILON: f32 = 0.0001;

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < EPSILON,
        "{actual} != {expected}"
    );
}

#[test]
fn small_and_big_follow_ua_relative_font_sizes_at_common_parent_sizes() {
    for (parent_size, expected_small, expected_big) in [
        (11.76, 9.8, 14.112),
        (16.0, 13.333333, 19.2),
        (20.0, 16.666667, 24.0),
        (24.0, 20.0, 28.8),
    ] {
        let mut parent = CssStyle::browser_default();
        parent.font_size = parent_size;
        let small = CssStyle::from_element("small", &[], &parent);
        let big = CssStyle::from_element("big", &[], &parent);
        if parent_size == 11.76 {
            assert_eq!(small.font_size, 9.8);
        } else {
            assert_close(small.font_size, expected_small);
        }
        assert_close(big.font_size, expected_big);
    }
}

#[test]
fn relative_line_height_recomputes_but_fixed_line_height_is_preserved() {
    let mut parent = CssStyle::browser_default();
    parent.font_size = 20.0;
    parent.line_height_factor = Some(1.7);
    parent.line_height = 34.0;
    let small = CssStyle::from_element("small", &[], &parent);
    let big = CssStyle::from_element("big", &[], &parent);
    assert_close(small.line_height, 20.0 / 1.2 * 1.7);
    assert_close(big.line_height, 20.0 * 1.2 * 1.7);

    let fixed_parent = CssStyle::from_element(
        "div",
        &[(
            "style".to_string(),
            "font-size:20px;line-height:20px".to_string(),
        )],
        &CssStyle::browser_default(),
    );
    let fixed_small = CssStyle::from_element("small", &[], &fixed_parent);
    assert_close(fixed_small.line_height, 20.0);
}

#[test]
fn nested_relative_tags_compose_and_author_font_size_wins() {
    let parent = CssStyle::from_element(
        "div",
        &[("style".to_string(), "font-size:24px".to_string())],
        &CssStyle::browser_default(),
    );
    let nested =
        CssStyle::from_element("small", &[], &CssStyle::from_element("small", &[], &parent));
    assert_close(nested.font_size, 16.666667);

    let inherited = CssStyle::from_element(
        "small",
        &[("style".to_string(), "font-size:inherit".to_string())],
        &parent,
    );
    assert_close(inherited.font_size, 24.0);
    let percentage = CssStyle::from_element(
        "small",
        &[("style".to_string(), "font-size:85%".to_string())],
        &parent,
    );
    assert_close(percentage.font_size, 20.4);
    let authored_px = CssStyle::from_element(
        "small",
        &[("style".to_string(), "font-size:12px".to_string())],
        &parent,
    );
    assert_eq!(authored_px.font_size, 12.0);
}
