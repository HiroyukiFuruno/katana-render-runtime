pub(super) fn is_non_rendered_tag(tag: &str) -> bool {
    matches!(
        tag,
        "head" | "link" | "meta" | "script" | "style" | "template" | "title"
    )
}
