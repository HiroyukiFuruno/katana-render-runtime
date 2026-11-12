use super::super::super::types::ElementPositioningContext;
use super::support::*;
use crate::renderer::backends::html_document::HtmlDocument;

const TABLE_HTML: &str = "<table style='border-collapse:collapse;width:240px'>\
    <thead id='head'><tr id='head-row'><th id='head-cell'>Header</th><th id='head-cell-2'>Other</th></tr></thead>\
    <tbody id='empty'></tbody>\
    <tbody id='body-a'><tr id='body-row-a'><td id='body-cell-a'>One</td><td id='body-cell-a-2'>Two</td></tr>\
    <tr id='body-row-a-2'><td id='body-cell-a-3'>Three</td></tr></tbody>\
    <tbody id='body-b'><tr id='body-row-b'><td id='body-cell-b'>Two</td></tr></tbody>\
    <tfoot id='foot'><tr id='foot-row'><td id='foot-cell'>Footer</td></tr></tfoot>\
    </table>";

#[test]
fn parsed_table_rows_and_groups_keep_painted_element_bounds() -> Result<(), String> {
    let mut document = HtmlDocument::parse(TABLE_HTML);
    let ids = TABLE_FIXTURE_IDS
        .map(|id| table_element_id(&mut document, id))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let layout = render_table_html(TABLE_HTML)?;
    assert_nonempty_elements_have_bounds(&layout, &ids)?;
    assert_group_bounds_and_order(&layout, &ids)?;
    assert_table_svg_ignores_dom_ids(&layout)?;
    Ok(())
}

#[test]
fn table_bounds_helpers_report_missing_and_unpainted_elements() -> Result<(), String> {
    let html = "<table><tbody id='group'><tr><td id='visible'>A</td></tr></tbody></table><template id='unpainted'>B</template>";
    let mut document = HtmlDocument::parse(html);
    let group = table_element_id(&mut document, "group")?;
    let visible = table_element_id(&mut document, "visible")?;
    let unpainted = table_element_id(&mut document, "unpainted")?;
    let layout = render_table_html(html)?;
    assert_eq!(
        table_element_id(&mut document, "missing").err().as_deref(),
        Some("fixture element #missing is missing")
    );
    let expected = format!("table element bounds for node {unpainted} are missing");
    assert_eq!(
        table_bounds(&layout, unpainted).err().as_deref(),
        Some(expected.as_str())
    );
    assert_eq!(
        assert_table_union(&layout, group, &[]).err().as_deref(),
        Some("table union has no members")
    );
    assert_eq!(
        assert_table_union(&layout, group, &[visible, unpainted])
            .err()
            .as_deref(),
        Some(expected.as_str())
    );
    Ok(())
}

#[rstest::rstest]
#[case::cell(
    "<td id='omitted' style='display:none;width:200px;background:red'>Omitted</td>",
    "",
    ""
)]
#[case::header("<th id='omitted' class='hide'>Omitted</th>", "", "")]
#[case::row("", "<tr id='omitted' class='hide'><td>Omitted</td></tr>", "")]
#[case::group(
    "",
    "",
    "<tbody id='omitted' class='hide'><tr><td>Omitted</td></tr></tbody>"
)]
fn display_none_table_parts_do_not_change_rendering(
    #[case] cell: &str,
    #[case] row: &str,
    #[case] group: &str,
) -> Result<(), String> {
    let source = table_with_optional_parts(cell, row, group);
    let reference = render_table_html(&table_with_optional_parts("", "", ""))?;
    let actual = render_table_html(&source)?;
    assert_eq!(actual.svg, reference.svg);
    let mut document = HtmlDocument::parse(&source);
    assert!(table_bounds(&actual, table_element_id(&mut document, "omitted")?).is_err());
    Ok(())
}

fn table_with_optional_parts(cell: &str, row: &str, group: &str) -> String {
    format!(
        "<style>.hide {{ display:none }}</style><table style='width:240px'><tbody><tr>{cell}<td>A</td><td>B</td></tr>{row}</tbody>{group}</table>"
    )
}

fn assert_nonempty_elements_have_bounds(
    layout: &super::super::super::types::LayoutResult,
    ids: &[u64],
) -> Result<(), String> {
    for (id, name) in ids.iter().zip(TABLE_FIXTURE_IDS) {
        if name == "empty" {
            continue;
        }
        let bounds = table_bounds(layout, *id)?;
        assert!(bounds.x.is_finite() && bounds.y.is_finite());
        assert!(bounds.width.is_finite() && bounds.height.is_finite());
        assert!(bounds.width > 0.0 && bounds.height > 0.0);
    }
    let empty_id = fixture_id(ids, "empty")?;
    assert!(
        layout
            .element_boxes
            .iter()
            .all(|bounds| bounds.node_id != empty_id)
    );
    Ok(())
}

fn assert_group_bounds_and_order(
    layout: &super::super::super::types::LayoutResult,
    ids: &[u64],
) -> Result<(), String> {
    assert_table_group_union(layout, ids, "head", &["head-cell", "head-cell-2"])?;
    assert_table_group_union(layout, ids, "head-row", &["head-cell", "head-cell-2"])?;
    assert_table_group_union(layout, ids, "body-row-a", &["body-cell-a", "body-cell-a-2"])?;
    assert_table_group_union(
        layout,
        ids,
        "body-a",
        &["body-cell-a", "body-cell-a-2", "body-cell-a-3"],
    )?;
    assert!(
        table_bounds(layout, fixture_id(ids, "body-b")?)?.y
            > table_bounds(layout, fixture_id(ids, "body-a")?)?.y
    );
    assert!(
        table_bounds(layout, fixture_id(ids, "foot")?)?.y
            > table_bounds(layout, fixture_id(ids, "body-b")?)?.y
    );
    Ok(())
}

fn assert_table_group_union(
    layout: &super::super::super::types::LayoutResult,
    ids: &[u64],
    group: &str,
    member_names: &[&str],
) -> Result<(), String> {
    let group_id = fixture_id(ids, group)?;
    let members = member_names
        .iter()
        .map(|name| fixture_id(ids, name))
        .collect::<Result<Vec<_>, _>>()?;
    assert_table_union(layout, group_id, &members)
}

fn assert_table_svg_ignores_dom_ids(
    layout: &super::super::super::types::LayoutResult,
) -> Result<(), String> {
    let without_ids = TABLE_FIXTURE_IDS
        .iter()
        .fold(TABLE_HTML.to_string(), |html, id| {
            html.replace(&format!(" id='{id}'"), "")
        });
    assert_eq!(layout.svg, render_table_html(&without_ids)?.svg);
    Ok(())
}

fn fixture_id(ids: &[u64], name: &str) -> Result<u64, String> {
    TABLE_FIXTURE_IDS
        .iter()
        .position(|candidate| *candidate == name)
        .and_then(|index| ids.get(index).copied())
        .ok_or_else(|| format!("fixture element id #{name} is missing"))
}

#[test]
fn table_element_bounds_keep_positioning_context_and_scroll_extent() -> Result<(), String> {
    assert_fixed_table_bounds()?;
    assert_absolute_table_bounds()?;
    Ok(())
}

fn assert_fixed_table_bounds() -> Result<(), String> {
    let html = "<div id='fixed-owner' style='position:fixed;top:900px'><table><tbody id='fixed-group'><tr id='fixed-row'><td id='fixed-cell'>Fixed</td></tr></tbody></table></div>";
    let mut document = HtmlDocument::parse(html);
    let ids = ["fixed-group", "fixed-row", "fixed-cell"]
        .map(|id| table_element_id(&mut document, id))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let layout = render_table_html(html)?;
    for id in ids {
        assert_eq!(
            table_bounds(&layout, id)?.positioning_context,
            ElementPositioningContext::FixedViewport
        );
    }
    assert_eq!(layout.content_height, TEST_VIEWPORT_HEIGHT as f32);
    Ok(())
}

fn assert_absolute_table_bounds() -> Result<(), String> {
    let html = "<div id='positioned-owner' style='position:relative;height:40px'><div id='absolute-owner' style='position:absolute;top:400px'><table><tbody id='absolute-group'><tr id='absolute-row'><td id='absolute-cell'>Absolute</td></tr></tbody></table></div></div>";
    let mut document = HtmlDocument::parse(html);
    let owner = table_element_id(&mut document, "positioned-owner")?;
    let ids = ["absolute-group", "absolute-row", "absolute-cell"]
        .map(|id| table_element_id(&mut document, id))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let layout = render_table_html(html)?;
    let expected = ElementPositioningContext::AbsoluteContainingBlock {
        owner_node_id: Some(owner),
        viewport_escape: false,
    };
    for id in ids {
        assert_eq!(table_bounds(&layout, id)?.positioning_context, expected);
    }
    let absolute = table_element_id(&mut HtmlDocument::parse(html), "absolute-owner")?;
    let bounds = table_bounds(&layout, absolute)?;
    assert!(layout.content_height <= bounds.y + bounds.height + 0.01);
    Ok(())
}
