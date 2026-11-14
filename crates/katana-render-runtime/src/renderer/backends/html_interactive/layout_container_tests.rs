use super::helpers::{
    accept_flow_result, container_height, horizontal_box_geometry, inline_container_width,
};
use super::{CssStyle, DetailsContext, HtmlBrowserViewport, HtmlDocumentNode, HtmlLayoutRenderer};
use std::collections::HashMap;

#[test]
fn flow_errors_are_recorded_at_the_container_start() {
    let mut layout_error = None;

    assert_eq!(
        accept_flow_result(&mut layout_error, Err("taffy failed".to_string()), 12.0),
        12.0
    );
    assert_eq!(layout_error, Some("taffy failed".to_string()));
}

#[test]
fn overflow_paint_without_an_element_owner_records_layout_error() {
    const EDGE: u32 = 40;
    let viewport = HtmlBrowserViewport {
        width: EDGE,
        height: EDGE,
        device_scale_factor: 1.0,
    };
    let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
    let mut style = CssStyle::browser_default();
    style.apply("overflow", "hidden");
    renderer.render_container(&[], 0.0, 0.0, EDGE as f32, &style, DetailsContext::NONE);
    assert_eq!(
        renderer.layout_error.as_deref(),
        Some("overflow paint clip has no owner")
    );
}

#[test]
fn inline_container_shrinks_to_text_content() {
    let children = [HtmlDocumentNode::Text("Compact label".to_string())];
    let mut style = CssStyle::browser_default();
    style.inline_block = true;

    assert!(inline_container_width(&children, 300.0, &style) < 120.0);
    assert_eq!(inline_container_width(&children, 40.0, &style), 40.0);
}

#[test]
fn inline_container_width_includes_margins_exactly_once() {
    let children = [HtmlDocumentNode::Text("Label".to_string())];
    let mut style = CssStyle::browser_default();
    style.inline_block = true;
    let without_margins = inline_container_width(&children, 300.0, &style);

    style.margin_left = 10.0;
    style.margin_right = 5.0;
    assert_eq!(
        inline_container_width(&children, 300.0, &style),
        without_margins + 15.0
    );
}

#[test]
fn inline_container_auto_width_includes_border_box_padding() {
    let children = [HtmlDocumentNode::Text("要件仕様 v1".to_string())];
    let mut style = CssStyle::browser_default();
    style.inline_block = true;
    style.padding_left = 10.0;
    style.padding_right = 10.0;
    style.border_width = 1.0;
    style.apply("border-style", "solid");

    let mut content_style = style.clone();
    content_style.padding_left = 0.0;
    content_style.padding_right = 0.0;
    content_style.border_width = 0.0;
    let content_width = inline_container_width(&children, 300.0, &content_style);

    assert_eq!(
        inline_container_width(&children, 300.0, &style),
        content_width + 22.0
    );
}

#[test]
fn container_height_is_clamped_by_max_height() {
    let mut style = CssStyle::browser_default();
    style.max_height = Some(20.0);

    assert_eq!(container_height(100.0, 0.0, &style), 20.0);
}

#[test]
fn horizontal_auto_margins_center_a_constrained_box() {
    let mut style = CssStyle::browser_default();
    style.max_width = Some(super::super::style::CssLength::Px(100.0));
    style.margin_left_auto = true;
    style.margin_right_auto = true;

    assert_eq!(horizontal_box_geometry(0.0, 320.0, &style), (110.0, 100.0));
}

#[test]
fn fixed_widths_overflow_narrow_containers_with_both_box_sizings() {
    let content_box = CssStyle::from_attributes(
        &[(
            "style".to_string(),
            "width:60px;padding:5px;border:1px solid".to_string(),
        )],
        &CssStyle::browser_default(),
    );
    let border_box = CssStyle::from_attributes(
        &[(
            "style".to_string(),
            "width:60px;box-sizing:border-box;padding:5px;border:1px solid".to_string(),
        )],
        &CssStyle::browser_default(),
    );

    assert_eq!(
        horizontal_box_geometry(0.0, 45.0, &content_box),
        (0.0, 72.0)
    );
    assert_eq!(horizontal_box_geometry(0.0, 45.0, &border_box), (0.0, 60.0));
}

#[test]
fn percentage_and_min_widths_can_overflow_narrow_containers() {
    let percentage = CssStyle::from_attributes(
        &[("style".to_string(), "width:150%".to_string())],
        &CssStyle::browser_default(),
    );
    let minimum = CssStyle::from_attributes(
        &[("style".to_string(), "min-width:60px".to_string())],
        &CssStyle::browser_default(),
    );

    assert_eq!(horizontal_box_geometry(0.0, 45.0, &percentage), (0.0, 67.5));
    assert_eq!(horizontal_box_geometry(0.0, 45.0, &minimum), (0.0, 60.0));
}

#[test]
fn max_width_still_bounds_fixed_width_and_auto_width_still_fits() {
    let fixed = CssStyle::from_attributes(
        &[("style".to_string(), "width:60px;max-width:50px".to_string())],
        &CssStyle::browser_default(),
    );
    let automatic = CssStyle::from_attributes(
        &[("style".to_string(), "max-width:80px".to_string())],
        &CssStyle::browser_default(),
    );

    assert_eq!(horizontal_box_geometry(0.0, 45.0, &fixed), (0.0, 50.0));
    assert_eq!(horizontal_box_geometry(0.0, 45.0, &automatic), (0.0, 45.0));
}
