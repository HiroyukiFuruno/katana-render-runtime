use super::super::super::html_document::{HtmlDocument, HtmlDocumentNode};
use super::super::layout::HtmlLayoutRenderer;
use super::super::style::{CssLength, CssStyle};
use super::{find_tag, item_style, measured_widths};
use crate::renderer::backends::html_browser::HtmlBrowserViewport;
use std::collections::HashMap;

const CONTAINER_WIDTH: f32 = 100.0;
const EXPLICIT_CHILD_WIDTH: f32 = 200.0;

#[test]
fn explicit_cross_width_overflows_flex_column_without_stretch() -> Result<(), String> {
    let (child, parent_style, child_style) = projected_flex_pair(
        "<section style=\"display:flex;flex-direction:column;align-items:flex-start;width:100px;background:#123456\"><div style=\"width:200px;height:20px;background:#dc2626\">fixed</div></section>",
    )?;

    assert_eq!(parent_style.background.as_deref(), Some("#123456"));
    assert_eq!(child_style.background.as_deref(), Some("#dc2626"));
    assert_eq!(child_style.width, Some(CssLength::Px(200.0)));
    assert_eq!(
        measured_widths(&[&child], &[child_style], CONTAINER_WIDTH, &parent_style),
        [EXPLICIT_CHILD_WIDTH]
    );
    Ok(())
}

#[test]
fn explicit_cross_width_is_preserved_when_column_stretches() -> Result<(), String> {
    let (child, parent_style, child_style) = projected_flex_pair(
        "<section style=\"display:flex;flex-direction:column;align-items:stretch;width:100px;background:#123456\"><div style=\"width:200px;height:20px;background:#dc2626\">fixed</div></section>",
    )?;

    assert_eq!(
        measured_widths(&[&child], &[child_style], CONTAINER_WIDTH, &parent_style),
        [EXPLICIT_CHILD_WIDTH]
    );
    Ok(())
}

#[test]
fn intrinsic_cross_width_still_shrinks_to_the_flex_container() -> Result<(), String> {
    let (child, parent_style, child_style) = projected_flex_pair(
        "<section style=\"display:flex;flex-direction:column;align-items:flex-start;width:100px;background:#123456\"><div style=\"height:20px;background:#dc2626\">an intrinsic item wider than its container</div></section>",
    )?;

    let measured = measured_widths(&[&child], &[child_style], CONTAINER_WIDTH, &parent_style)[0];
    assert_eq!(measured, CONTAINER_WIDTH);
    Ok(())
}

#[test]
fn omitted_align_items_matches_stretch_for_intrinsic_column_item() -> Result<(), String> {
    let markups = [
        "<section style=\"display:flex;flex-direction:column;width:100px\"><div style=\"height:20px;background:#dc2626\">an intrinsic item wider than its container</div></section>",
        "<section style=\"display:flex;flex-direction:column;align-items:stretch;width:100px\"><div style=\"height:20px;background:#dc2626\">an intrinsic item wider than its container</div></section>",
        "<section style=\"display:flex;flex-direction:column;align-items:normal;width:100px\"><div style=\"height:20px;background:#dc2626\">an intrinsic item wider than its container</div></section>",
        "<section style=\"display:flex;flex-direction:column;align-items:flex-start;width:100px\"><div style=\"height:20px;background:#dc2626\">an intrinsic item wider than its container</div></section>",
    ];
    let widths = markups
        .into_iter()
        .map(|markup| {
            let (child, parent, child_style) = projected_flex_pair(markup)?;
            Ok(measured_widths(&[&child], &[child_style], CONTAINER_WIDTH, &parent)[0])
        })
        .collect::<Result<Vec<_>, String>>()?;
    assert_eq!(widths[0], widths[1]);
    assert_eq!(widths[0], widths[2]);
    assert_eq!(widths[3], CONTAINER_WIDTH);
    assert_eq!(widths[0], CONTAINER_WIDTH);
    let svgs = markups
        .into_iter()
        .map(rendered_flex_svg)
        .collect::<Result<Vec<_>, String>>()?;
    assert_eq!(svgs[0], svgs[1]);
    assert_eq!(svgs[0], svgs[2]);
    assert_eq!(svgs[0], svgs[3]);
    assert!(svgs[0].contains(r##"<rect x="8" y="8" width="100" height="20" fill="#dc2626"/>"##));

    assert_intrinsic_reverse_column_width()?;
    Ok(())
}

fn assert_intrinsic_reverse_column_width() -> Result<(), String> {
    let reverse_omitted = projected_flex_pair(
        "<section style=\"display:flex;flex-direction:column-reverse;width:100px\"><div style=\"height:20px;background:#dc2626\">an intrinsic item wider than its container</div></section>",
    )?;
    let reverse_normal = projected_flex_pair(
        "<section style=\"display:flex;flex-direction:column-reverse;align-items:normal;width:100px\"><div style=\"height:20px;background:#dc2626\">an intrinsic item wider than its container</div></section>",
    )?;
    let reverse_omitted_width = measured_widths(
        &[&reverse_omitted.0],
        &[reverse_omitted.2],
        CONTAINER_WIDTH,
        &reverse_omitted.1,
    )[0];
    let reverse_normal_width = measured_widths(
        &[&reverse_normal.0],
        &[reverse_normal.2],
        CONTAINER_WIDTH,
        &reverse_normal.1,
    )[0];
    assert_eq!(reverse_normal_width, reverse_omitted_width);
    assert_eq!(reverse_normal_width, CONTAINER_WIDTH);
    Ok(())
}

#[test]
fn row_stretch_expands_auto_height_children_to_the_explicit_cross_size() -> Result<(), String> {
    let svg = rendered_flex_svg(
        "<section style=\"display:flex;align-items:stretch;width:300px;height:100px;background:red\"><div style=\"width:150px;background:#008000\"></div><div style=\"width:150px;background:#008000\"></div></section>",
    )?;
    assert!(
        svg.contains(r##"<rect x="8" y="8" width="150" height="100" fill="#008000"/>"##)
            && svg.contains(r##"<rect x="158" y="8" width="150" height="100" fill="#008000"/>"##),
        "{svg}"
    );
    Ok(())
}

#[test]
fn row_default_and_reverse_stretch_auto_children_to_the_cross_size() -> Result<(), String> {
    let default_svg = rendered_flex_svg(
        "<section style=\"display:flex;width:300px;height:100px\"><div style=\"width:150px;background:#008000\"></div><div style=\"width:150px;background:#008000\"></div></section>",
    )?;
    let reverse_svg = rendered_flex_svg(
        "<section style=\"display:flex;flex-direction:row-reverse;width:300px;height:100px\"><div style=\"width:150px;background:#008000\"></div><div style=\"width:150px;background:#008000\"></div></section>",
    )?;
    let normal_svg = rendered_flex_svg(
        "<section style=\"display:flex;align-items:normal;justify-content:normal;width:300px;height:100px\"><div style=\"width:150px;background:#008000\"></div><div style=\"width:150px;background:#008000\"></div></section>",
    )?;
    let normal_reverse_svg = rendered_flex_svg(
        "<section style=\"display:flex;flex-direction:row-reverse;align-items:normal;justify-content:normal;width:300px;height:100px\"><div style=\"width:150px;background:#008000\"></div><div style=\"width:150px;background:#008000\"></div></section>",
    )?;
    assert_stretched_green_rects(&default_svg);
    assert_stretched_green_rects(&reverse_svg);
    assert_stretched_green_rects(&normal_svg);
    assert_stretched_green_rects(&normal_reverse_svg);
    assert_eq!(normal_svg, default_svg);
    Ok(())
}

#[test]
fn row_stretch_preserves_explicit_child_height_and_flex_start_auto_height() -> Result<(), String> {
    let fixed_svg = rendered_flex_svg(
        "<section style=\"display:flex;width:300px;height:100px\"><div style=\"width:150px;height:20px;background:#008000\"></div><div style=\"width:150px;height:20px;background:#008000\"></div></section>",
    )?;
    let start_svg = rendered_flex_svg(
        "<section style=\"display:flex;align-items:flex-start;width:300px;height:100px\"><div style=\"width:150px;background:#008000\"></div><div style=\"width:150px;background:#008000\"></div></section>",
    )?;
    assert!(fixed_svg.contains(r##"<rect x="8" y="8" width="150" height="20" fill="#008000"/>"##));
    assert!(
        fixed_svg.contains(r##"<rect x="158" y="8" width="150" height="20" fill="#008000"/>"##)
    );
    assert!(start_svg.contains(r##"<rect x="8" y="8" width="150" height="1" fill="#008000"/>"##));
    assert!(start_svg.contains(r##"<rect x="158" y="8" width="150" height="1" fill="#008000"/>"##));
    Ok(())
}

#[test]
fn wrapped_auto_child_keeps_remeasured_content_height() -> Result<(), String> {
    let wrapped_svg = rendered_flex_svg(
        "<section style=\"display:flex;flex-wrap:wrap;width:120px\"><div style=\"width:120px;background:#008000\">a long line that wraps into multiple lines</div></section>",
    )?;
    let block_svg = rendered_flex_svg(
        "<section style=\"width:120px\"><div style=\"width:120px;background:#008000\">a long line that wraps into multiple lines</div></section>",
    )?;
    let wrapped_heights = green_rect_heights(&wrapped_svg);
    let block_heights = green_rect_heights(&block_svg);
    assert_eq!(wrapped_heights, block_heights);
    assert!(wrapped_heights.first().is_some_and(|height| *height > 1.0));
    Ok(())
}

#[test]
fn explicit_cross_width_reaches_colored_svg_bounds() -> Result<(), String> {
    let svg = rendered_flex_svg(
        "<section style=\"display:flex;flex-direction:column;align-items:flex-start;width:100px;background:#123456\"><div style=\"width:200px;height:20px;background:#dc2626\">fixed</div></section>",
    )?;
    assert!(
        svg.contains(r##"<rect x="8" y="8" width="200" height="20" fill="#dc2626"/>"##),
        "{svg}"
    );
    Ok(())
}

#[test]
fn explicit_cross_width_respects_min_and_max_constraints() -> Result<(), String> {
    let (child, parent, min_style) = projected_flex_pair(
        "<section style=\"display:flex;flex-direction:column;align-items:flex-start;width:100px\"><div style=\"width:80px;min-width:140px;height:20px\">min</div></section>",
    )?;
    assert_eq!(
        measured_widths(&[&child], &[min_style], CONTAINER_WIDTH, &parent),
        [140.0]
    );

    let (child, parent, max_style) = projected_flex_pair(
        "<section style=\"display:flex;flex-direction:column;align-items:flex-start;width:100px\"><div style=\"width:200px;max-width:120px;height:20px\">max</div></section>",
    )?;
    assert_eq!(
        measured_widths(&[&child], &[max_style], CONTAINER_WIDTH, &parent),
        [120.0]
    );
    Ok(())
}

#[test]
fn explicit_width_is_unchanged_for_row_and_block_parents() -> Result<(), String> {
    for parent_display in ["display:flex;flex-direction:row", "display:block"] {
        let markup = format!(
            "<section style=\"{parent_display};width:100px\"><div style=\"width:200px;height:20px\">fixed</div></section>"
        );
        let (child, parent, child_style) = projected_flex_pair(&markup)?;
        assert_eq!(
            measured_widths(&[&child], &[child_style], CONTAINER_WIDTH, &parent),
            [EXPLICIT_CHILD_WIDTH]
        );
    }
    Ok(())
}

#[test]
fn centered_column_cross_size_reaches_public_svg_geometry() -> Result<(), String> {
    let svg = rendered_flex_svg(
        "<section style=\"display:flex;flex-direction:column;align-items:center;width:100px\"><div style=\"width:40px;height:20px;background:#dc2626\">center</div></section>",
    )?;
    assert!(
        svg.contains(r##"<rect x="38" y="8" width="40" height="20" fill="#dc2626"/>"##),
        "{svg}"
    );
    Ok(())
}

fn projected_flex_pair(markup: &str) -> Result<(HtmlDocumentNode, CssStyle, CssStyle), String> {
    let document = HtmlDocument::parse(markup);
    let nodes = document.interactive_nodes_with_styles(&HashMap::new());
    let parent = nodes
        .iter()
        .find_map(|node| find_tag(node, "section"))
        .ok_or("flex container projection missing")?;
    let child = find_tag(parent, "div").ok_or("flex child projection missing")?;
    let inherited = CssStyle::browser_default();
    let parent_style = item_style(parent, &inherited, CONTAINER_WIDTH, 1);
    let child_style = item_style(child, &parent_style, CONTAINER_WIDTH, 1);
    Ok((child.clone(), parent_style, child_style))
}

fn rendered_flex_svg(markup: &str) -> Result<String, String> {
    let document = HtmlDocument::parse(markup);
    let nodes = document.interactive_nodes_with_styles(&HashMap::new());
    let viewport = HtmlBrowserViewport::new(320, 120, 1.0).map_err(|error| error.to_string())?;
    Ok(HtmlLayoutRenderer::render(&nodes, viewport, 0.0, &HashMap::new(), None)?.svg)
}

fn assert_stretched_green_rects(svg: &str) {
    assert!(svg.contains(r##"<rect x="8" y="8" width="150" height="100" fill="#008000"/>"##));
    assert!(svg.contains(r##"<rect x="158" y="8" width="150" height="100" fill="#008000"/>"##));
}

fn green_rect_heights(svg: &str) -> Vec<f32> {
    svg.split("<rect ")
        .filter(|rect| rect.contains("fill=\"#008000\""))
        .filter_map(|rect| {
            rect.split("height=\"")
                .nth(1)
                .and_then(|value| value.split('"').next())
                .and_then(|value| value.parse().ok())
        })
        .collect()
}
