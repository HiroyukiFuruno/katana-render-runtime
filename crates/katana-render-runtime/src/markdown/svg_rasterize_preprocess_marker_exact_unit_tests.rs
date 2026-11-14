use super::exact::rewrite_member;

#[test]
fn marker_selector_exact_member_fails_closed_for_unsupported_css() {
    for selector in [
        r#"[cl\61 ss="extension"]"#,
        r#"[id="actual\000022quote"]"#,
        r#"[id="actual\26-end"][id="actual\26-end"]"#,
        r#"[id="actual\26-end" i]"#,
        r#"[id=actual\26-end]"#,
        r#"[id~="actual\26-end"]"#,
    ] {
        assert!(
            rewrite_member(selector, usize::MAX).is_none(),
            "expected fail-closed for {selector}"
        );
    }
}

#[test]
fn marker_selector_exact_member_preserves_escape_and_output_budget() {
    let selector = r#"[class="extension"][id="actual\26-end"]"#;
    assert!(rewrite_member(selector, selector.len()).is_none());
    assert!(
        rewrite_member(selector, selector.len() + 2).is_some_and(|normalized| {
            normalized == r#"[class="extension"][id="actual&amp;-end"]"#
        }),
        "expected XML-safe selector within the larger output budget"
    );
}
