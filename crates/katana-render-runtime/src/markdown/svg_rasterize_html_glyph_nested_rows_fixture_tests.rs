use super::font_file_fixture;
use resvg::usvg::{Options, Tree};
use std::sync::Arc;

pub(super) struct NestedRowsSpec<'a> {
    pub(super) anchor: &'a str,
    pub(super) font_size: &'a str,
    pub(super) transform: &'a str,
    pub(super) anchor_x: &'a str,
    pub(super) second_row: bool,
}

pub(super) fn nested_rows_tree_with_anchor(
    fixture: &font_file_fixture::FontFileFixture,
    attributes: &str,
    row_attributes: &str,
    spec: NestedRowsSpec<'_>,
) -> Result<Tree, String> {
    let second_row = make_second_row(&spec, row_attributes);
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg'><text id='run' font-family='Noto Sans' font-size='{}' transform='{}' {attributes}><tspan x='{}' y='0' dy='1.1em' text-anchor='{}'><tspan>A</tspan><tspan xml:space='preserve'>V customers </tspan></tspan>{second_row}</text></svg>",
        spec.font_size, spec.transform, spec.anchor_x, spec.anchor
    );
    Tree::from_str(
        &svg,
        &Options {
            fontdb: Arc::new(fixture.database.clone()),
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())
}

fn make_second_row(spec: &NestedRowsSpec<'_>, row_attributes: &str) -> String {
    if !spec.second_row {
        return String::new();
    }
    format!(
        "<tspan x='{}' y='1em' dy='1.1em' text-anchor='{}' {row_attributes}><tspan>information</tspan><tspan> about</tspan></tspan>",
        spec.anchor_x, spec.anchor
    )
}
