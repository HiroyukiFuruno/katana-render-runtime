use std::ops::Range;

pub(super) struct SourceEdit {
    range: Range<usize>,
    replacement: String,
}

impl SourceEdit {
    pub(super) fn new(range: Range<usize>, replacement: String) -> Self {
        Self { range, replacement }
    }
}

pub(super) fn apply_source_edits(
    source: &str,
    mut edits: Vec<SourceEdit>,
    allow_empty_ranges: bool,
) -> String {
    edits.sort_by_key(|edit| (edit.range.start, edit.range.end));
    if edits.iter().any(|edit| {
        let valid_order = if allow_empty_ranges {
            edit.range.start <= edit.range.end
        } else {
            edit.range.start < edit.range.end
        };
        !valid_order || source.get(edit.range.clone()).is_none()
    }) || edits
        .windows(2)
        .any(|pair| pair[0].range.end > pair[1].range.start)
    {
        return source.to_owned();
    }
    let mut output = source.to_owned();
    for edit in edits.into_iter().rev() {
        output.replace_range(edit.range, &edit.replacement);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{SourceEdit, apply_source_edits};

    #[test]
    fn allows_empty_insertions_only_when_requested() {
        let insertion = vec![SourceEdit::new(1..1, "x".to_owned())];
        assert_eq!(apply_source_edits("ab", insertion, true), "axb");

        let insertion = vec![SourceEdit::new(1..1, "x".to_owned())];
        assert_eq!(apply_source_edits("ab", insertion, false), "ab");
    }

    #[test]
    fn invalid_utf8_boundaries_and_overlaps_preserve_original_source() {
        let invalid_boundary = vec![SourceEdit::new(0..1, "x".to_owned())];
        assert_eq!(apply_source_edits("é", invalid_boundary, false), "é");

        let overlapping = vec![
            SourceEdit::new(0..2, "x".to_owned()),
            SourceEdit::new(1..3, "y".to_owned()),
        ];
        assert_eq!(apply_source_edits("abc", overlapping, false), "abc");
    }

    #[test]
    fn insertion_at_replacement_start_is_applied_before_replacement() {
        let edits = vec![
            SourceEdit::new(0..1, "A".to_owned()),
            SourceEdit::new(0..0, "x".to_owned()),
        ];
        assert_eq!(apply_source_edits("ab", edits, true), "xAb");
    }
}
