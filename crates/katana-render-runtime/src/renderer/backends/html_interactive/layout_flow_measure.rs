use super::super::html_document::HtmlDocumentNode;
use super::constants::MIN_LAYOUT_WIDTH;
use super::layout_grid_measure::grid_track_widths;
use super::style::{CssLength, CssStyle};
use taffy::geometry::{Rect, Size};
use taffy::prelude::{Display, Style};
use taffy::style_helpers::{auto, length};

#[path = "layout_flow_cross_size.rs"]
mod cross_size;
#[path = "layout_intrinsic.rs"]
mod intrinsic;

pub(super) use intrinsic::{intrinsic_text_width, is_layout_item, min_content_text_width};

pub(super) fn leaf_style(style: CssStyle, width: f32, height: f32, available_width: f32) -> Style {
    Style {
        display: Display::Block,
        size: Size {
            width: length(width),
            height: length(height),
        },
        padding: leaf_padding(&style),
        border: leaf_border(&style),
        min_size: Size {
            width: style
                .minimum_outer_width(available_width)
                .map_or_else(auto, length),
            height: length(leaf_minimum_height(&style, height)),
        },
        flex_grow: style.flex_grow,
        flex_shrink: style.flex_shrink,
        flex_basis: match style.flex_basis {
            Some(CssLength::Px(value)) => length(style.flex_basis_outer_width(value)),
            Some(CssLength::Percent(value)) => {
                length(style.flex_basis_outer_width(available_width * value))
            }
            None => auto(),
        },
        ..Style::default()
    }
}

pub(super) fn leaf_style_for_flow(
    style: CssStyle,
    width: f32,
    height: f32,
    available_width: f32,
    parent: &CssStyle,
) -> Style {
    let auto_height = style.height.is_none()
        && parent.display == Display::Flex
        && matches!(
            parent.flex_direction,
            taffy::style::FlexDirection::Row | taffy::style::FlexDirection::RowReverse
        )
        && matches!(
            parent.align_items,
            None | Some(taffy::style::AlignItems::STRETCH | taffy::style::AlignItems::NORMAL)
        );
    let mut leaf = leaf_style(style, width, height, available_width);
    if auto_height {
        leaf.size.height = auto();
    }
    leaf
}

fn leaf_padding(style: &CssStyle) -> Rect<taffy::style::LengthPercentage> {
    Rect {
        left: length(style.padding_left),
        right: length(style.padding_right),
        top: length(style.padding_top),
        bottom: length(style.padding_bottom),
    }
}

fn leaf_border(style: &CssStyle) -> Rect<taffy::style::LengthPercentage> {
    Rect {
        left: length(style.border_left_width()),
        right: length(style.border_right_width()),
        top: length(style.border_top_width()),
        bottom: length(style.border_bottom_width()),
    }
}

pub(super) fn assign_leaf_height(style: &mut Style, css_style: &CssStyle, height: f32) {
    style.size.height = length(height);
    style.min_size.height = length(leaf_minimum_height(css_style, height));
}

fn leaf_minimum_height(style: &CssStyle, measured_height: f32) -> f32 {
    if style.automatic_min_height {
        measured_height
    } else {
        style.minimum_outer_height() + style.margin_top + style.margin_bottom
    }
    .max(0.0)
}

pub(super) fn item_style(
    node: &HtmlDocumentNode,
    inherited: &CssStyle,
    _width: f32,
    _count: usize,
) -> CssStyle {
    match node {
        HtmlDocumentNode::Element {
            tag,
            attributes,
            children,
            ..
        } => {
            let mut style = CssStyle::from_element(tag, attributes, inherited);
            style.resolve_text_line_height(children);
            style
        }
        HtmlDocumentNode::Text(_) => inherited.inherited_text_style(),
    }
}

pub(super) fn measured_widths(
    nodes: &[&HtmlDocumentNode],
    styles: &[CssStyle],
    width: f32,
    parent: &CssStyle,
) -> Vec<f32> {
    if stretches_flex_columns(parent) {
        return cross_size::stretched_column_widths(styles, width);
    }
    if parent.display != Display::Grid {
        return cross_size::non_grid_measured_widths(nodes, styles, width, parent);
    }

    let track_widths = grid_track_widths(nodes, styles, width, parent);
    styles
        .iter()
        .enumerate()
        .map(|(index, style)| {
            let measured = style
                .explicit_width(width)
                .unwrap_or(track_widths[index % track_widths.len()])
                .min(width);
            cross_size::enforce_min_width(style, measured, width).max(MIN_LAYOUT_WIDTH)
        })
        .collect()
}

fn stretches_flex_columns(style: &CssStyle) -> bool {
    style.display == Display::Flex
        && matches!(
            style.flex_direction,
            taffy::style::FlexDirection::Column | taffy::style::FlexDirection::ColumnReverse
        )
        && matches!(
            style.align_items,
            None | Some(taffy::style::AlignItems::STRETCH | taffy::style::AlignItems::NORMAL)
        )
}

#[cfg(test)]
fn find_tag<'a>(node: &'a HtmlDocumentNode, expected: &str) -> Option<&'a HtmlDocumentNode> {
    match node {
        HtmlDocumentNode::Element { tag, children, .. } => {
            if tag == expected {
                return Some(node);
            }
            children.iter().find_map(|child| find_tag(child, expected))
        }
        HtmlDocumentNode::Text(_) => None,
    }
}

#[cfg(test)]
#[path = "tests/html_flex_cross_size.rs"]
mod html_flex_cross_size;

#[cfg(test)]
mod tests {
    use super::super::style::{CssGridTrack, CssGridTrackBreadth};
    use super::intrinsic::intrinsic_layout_width;
    use super::{CssLength, CssStyle, Display, intrinsic_text_width, is_layout_item};
    use super::{find_tag, item_style, measured_widths};
    use crate::renderer::backends::html_document::{HtmlDocument, HtmlDocumentNode};
    use std::collections::HashMap;

    #[test]
    fn flow_helpers_ignore_formatting_whitespace_and_measure_text() {
        let whitespace = HtmlDocumentNode::Text("  \n ".to_string());
        let text = HtmlDocumentNode::Text("abcd".to_string());
        let japanese = HtmlDocumentNode::Text("案件概要".to_string());

        assert!(!is_layout_item(&whitespace));
        assert!(is_layout_item(&text));
        assert!(intrinsic_text_width(&text, &CssStyle::browser_default()) > 30.0);
        assert!(
            intrinsic_text_width(&japanese, &CssStyle::browser_default())
                > intrinsic_text_width(&text, &CssStyle::browser_default())
        );
    }

    #[test]
    fn parsed_inline_content_uses_real_projection_for_intrinsic_measurement() -> Result<(), String>
    {
        let short = projected_width("<section><p>本文</p></section>")?;
        let long = projected_width("<section><p>案件概要と長い説明</p></section>")?;
        assert!(long > short, "long={long}, short={short}");
        Ok(())
    }

    #[test]
    fn parsed_whitespace_and_inline_child_keep_layout_content() -> Result<(), String> {
        let empty = projected_width("<section><p> \n </p></section>")?;
        let collapsed = projected_width("<section><p> 本文 </p></section>")?;
        let repeated = projected_width("<section><p> \n 本文 \n </p></section>")?;
        let plain = projected_width("<section><p>本文</p></section>")?;
        let inline = projected_width("<section><p><span>本文</span></p></section>")?;
        assert_eq!(empty, 0.0);
        assert!(collapsed > empty);
        assert!((repeated - collapsed).abs() <= f32::EPSILON);
        assert!(
            (inline - plain).abs() <= f32::EPSILON,
            "inline={inline}, plain={plain}"
        );
        Ok(())
    }

    #[test]
    fn parsed_column_flex_nonstretch_clamps_cross_size() -> Result<(), String> {
        let document = HtmlDocument::parse(
            "<section style=\"display:flex;flex-direction:column;align-items:flex-start\">section prefix<div>unrelated text</div><p id=target>an unbroken cross-axis item with intrinsic width</p></section>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let section = nodes
            .iter()
            .find_map(|node| find_tag(node, "section"))
            .ok_or("section projection missing")?;
        let paragraph = find_tag(section, "p").ok_or("paragraph projection missing")?;
        let inherited = CssStyle::browser_default();
        let container = item_style(section, &inherited, 40.0, 1);
        let child = item_style(paragraph, &container, 40.0, 1);
        let items = [paragraph];
        let styles = [child];
        assert_eq!(measured_widths(&items, &styles, 40.0, &container), [40.0]);

        let mut block = container;
        block.display = Display::Block;
        assert!(measured_widths(&items, &styles, 40.0, &block)[0] > 40.0);
        Ok(())
    }

    fn projected_width(markup: &str) -> Result<f32, String> {
        let document = HtmlDocument::parse(markup);
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let node = nodes
            .iter()
            .find_map(|node| find_tag(node, "p"))
            .ok_or("parsed section must be projected")?;
        let style = item_style(node, &CssStyle::browser_default(), 48.0, 1);
        assert!(
            is_layout_item(node),
            "a projected paragraph is a layout item"
        );
        Ok(intrinsic_layout_width(node, &style, 48.0))
    }

    fn parsed_grid_width(markup: &str) -> Result<(f32, Option<taffy::style::AlignItems>), String> {
        let document = HtmlDocument::parse(markup);
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let grid = nodes
            .iter()
            .find_map(|node| find_tag(node, "section"))
            .ok_or("parsed grid section missing")?;
        let child = find_tag(grid, "p").ok_or("parsed grid item missing")?;
        let parent_style = item_style(grid, &CssStyle::browser_default(), 80.0, 1);
        let child_style = item_style(child, &parent_style, 80.0, 1);
        let width = measured_widths(&[child], &[child_style], 80.0, &parent_style)
            .first()
            .copied()
            .ok_or("parsed grid item width missing")?;
        Ok((width, parent_style.align_items))
    }

    #[test]
    fn text_and_grid_items_receive_deterministic_measurements() {
        let text = HtmlDocumentNode::Text("item".to_string());
        let mut inherited = CssStyle::browser_default();
        inherited.font_family = "Noto Sans, sans-serif".to_string();
        let text_style = item_style(&text, &inherited, 100.0, 2);
        assert_eq!(text_style.width, None);

        let mut grid = CssStyle::browser_default();
        grid.display = Display::Grid;
        grid.grid_template_columns = vec![CssGridTrack::Fraction(1.0); 2];
        grid.gap = 10.0;
        let items = [&text, &text];
        let styles = [inherited.clone(), inherited.clone()];
        assert_eq!(measured_widths(&items, &styles, 100.0, &grid), [45.0, 45.0]);
        assert!(measured_widths(&items, &styles, 100.0, &inherited)[0] > 30.0);

        let mut column = inherited.clone();
        column.display = Display::Flex;
        column.flex_direction = taffy::style::FlexDirection::Column;
        assert_eq!(
            measured_widths(&items, &styles, 100.0, &column),
            [100.0, 100.0]
        );
    }

    #[test]
    fn parsed_grid_normal_matches_omission_for_fixed_and_auto_tracks() -> Result<(), String> {
        for track in ["80px", "auto"] {
            let omitted = parsed_grid_width(&format!(
                "<section style='display:grid;grid-template-columns:{track};width:80px'><p>an intrinsic grid item</p></section>"
            ))?;
            let normal = parsed_grid_width(&format!(
                "<section style='display:grid;grid-template-columns:{track};align-items:normal;width:80px'><p>an intrinsic grid item</p></section>"
            ))?;

            assert_eq!(omitted.1, None);
            assert_eq!(normal.1, Some(taffy::style::AlignItems::NORMAL));
            assert_eq!(normal.0, omitted.0, "track={track}");
        }
        Ok(())
    }

    #[test]
    fn parsed_grid_measurement_rejects_missing_section_and_item() {
        for (html, expected) in [
            ("<main><p>item</p></main>", "parsed grid section missing"),
            (
                "<section style='display:grid'><div>item</div></section>",
                "parsed grid item missing",
            ),
        ] {
            assert_eq!(parsed_grid_width(html).err().as_deref(), Some(expected));
        }
    }

    #[test]
    fn aligned_column_flex_items_fit_the_available_cross_axis() {
        let text = HtmlDocumentNode::Text("a long unbroken cross-axis item".to_string());
        let inherited = CssStyle::browser_default();
        let mut column = inherited.clone();
        column.display = Display::Flex;
        column.align_items = Some(taffy::style::AlignItems::START);
        for direction in [
            taffy::style::FlexDirection::Column,
            taffy::style::FlexDirection::ColumnReverse,
        ] {
            column.flex_direction = direction;
            assert_eq!(
                measured_widths(&[&text], std::slice::from_ref(&inherited), 40.0, &column),
                [40.0]
            );
        }
    }

    #[test]
    fn anonymous_flex_text_inherits_typography_without_the_container_box() {
        let text = HtmlDocumentNode::Text("1".to_string());
        let mut container = CssStyle::browser_default();
        container.display = Display::Flex;
        container.width = Some(CssLength::Px(46.0));
        container.height = Some(46.0);
        container.min_width = Some(CssLength::Px(46.0));
        container.padding_left = 7.0;
        container.margin_left = 9.0;
        container.flex_grow = 1.0;
        container.background = Some("#1e3a8a".to_string());
        container.font_size = 20.0;
        container.font_weight = 700;
        container.color = "#ffffff".to_string();

        let anonymous = item_style(&text, &container, 46.0, 1);

        assert_anonymous_box_defaults(&anonymous);
        assert_inherited_typography(&anonymous);
    }

    fn assert_anonymous_box_defaults(anonymous: &CssStyle) {
        assert_eq!(anonymous.width, None);
        assert_eq!(anonymous.height, None);
        assert_eq!(anonymous.min_width, None);
        assert_eq!(anonymous.padding_left, 0.0);
        assert_eq!(anonymous.margin_left, 0.0);
        assert_eq!(anonymous.flex_grow, 0.0);
        assert_eq!(anonymous.background, None);
    }

    fn assert_inherited_typography(anonymous: &CssStyle) {
        assert_eq!(anonymous.font_size, 20.0);
        assert_eq!(anonymous.font_weight, 700);
        assert_eq!(anonymous.color, "#ffffff");
    }

    #[test]
    fn leaf_style_forwards_typed_flex_basis_to_taffy() {
        let mut style = CssStyle::browser_default();
        style.flex_basis = Some(CssLength::Percent(0.0));
        style.padding_left = 8.0;
        style.padding_right = 8.0;
        let leaf = super::leaf_style(style, 80.0, 20.0, 200.0);
        assert_eq!(leaf.flex_basis, taffy::style_helpers::length(16.0_f32));

        let mut pixels = CssStyle::browser_default();
        pixels.flex_basis = Some(CssLength::Px(24.0));
        pixels.border_width = 1.0;
        pixels.apply("border-style", "solid");
        let leaf = super::leaf_style(pixels, 80.0, 20.0, 200.0);
        assert_eq!(leaf.flex_basis, taffy::style_helpers::length(26.0_f32));

        let leaf = super::leaf_style(CssStyle::browser_default(), 80.0, 20.0, 200.0);
        assert_eq!(leaf.flex_basis, taffy::style_helpers::auto());
    }

    #[test]
    fn padded_flex_shrink_matches_independent_chrome_measurement() -> Result<(), String> {
        use taffy::prelude::{TaffyMaxContent, TaffyTree};
        let mut tree: TaffyTree<()> = TaffyTree::new();
        let plain = super::leaf_style(CssStyle::browser_default(), 720.0, 20.0, 640.0);
        let first = tree.new_leaf(plain).map_err(test_error)?;
        let mut badge = CssStyle::browser_default();
        badge.padding_left = 10.0;
        badge.padding_right = 10.0;
        let second = tree
            .new_leaf(super::leaf_style(badge, 80.0, 20.0, 640.0))
            .map_err(test_error)?;
        let root = tree
            .new_with_children(padded_flex_parent(), &[first, second])
            .map_err(test_error)?;
        tree.disable_rounding();
        tree.compute_layout(root, taffy::geometry::Size::MAX_CONTENT)
            .map_err(test_error)?;
        let width = tree.layout(second).map_err(test_error)?.size.width;
        assert!((width - 65.84375).abs() <= 1.0 / 64.0, "width {width}");
        Ok(())
    }

    fn padded_flex_parent() -> taffy::Style {
        let defaults: taffy::Style = taffy::Style::default();
        taffy::Style {
            display: Display::Flex,
            size: taffy::geometry::Size {
                width: taffy::style_helpers::length(640.0_f32),
                ..defaults.size
            },
            gap: taffy::geometry::Size {
                width: taffy::style_helpers::length(24.0_f32),
                ..defaults.gap
            },
            ..defaults
        }
    }

    #[test]
    fn explicit_stretch_alignment_expands_flex_column_items() {
        let text = HtmlDocumentNode::Text("item".to_string());
        let styles = [CssStyle::browser_default()];
        let mut column = CssStyle::browser_default();
        column.display = Display::Flex;
        column.flex_direction = taffy::style::FlexDirection::Column;
        column.align_items = Some(taffy::style::AlignItems::STRETCH);

        assert_eq!(measured_widths(&[&text], &styles, 120.0, &column), [120.0]);
    }

    #[test]
    fn flex_start_column_shrink_to_fit_is_clamped_to_available_width() {
        let text = HtmlDocumentNode::Text("a deliberately long intrinsic flex item".to_string());
        let styles = [CssStyle::browser_default()];
        let mut column = CssStyle::browser_default();
        column.display = Display::Flex;
        column.flex_direction = taffy::style::FlexDirection::Column;
        column.align_items = Some(taffy::style::AlignItems::FLEX_START);

        assert_eq!(measured_widths(&[&text], &styles, 120.0, &column), [120.0]);
    }

    #[test]
    fn reassigned_leaf_height_updates_size_and_automatic_minimum() {
        let style = CssStyle::browser_default();
        let mut leaf = super::leaf_style(style.clone(), 80.0, 20.0, 200.0);
        super::assign_leaf_height(&mut leaf, &style, 64.0);
        assert_eq!(leaf.size.height, taffy::style_helpers::length(64.0_f32));
        assert_eq!(leaf.min_size.height, taffy::style_helpers::length(64.0_f32));

        let mut explicit = style;
        explicit.automatic_min_height = false;
        explicit.min_height = 12.0;
        super::assign_leaf_height(&mut leaf, &explicit, 64.0);
        assert_eq!(leaf.min_size.height, taffy::style_helpers::length(12.0_f32));
    }

    #[test]
    fn grid_measurements_preserve_fixed_intrinsic_and_fraction_tracks() {
        let first = HtmlDocumentNode::Text("Menu".to_string());
        let second = HtmlDocumentNode::Text("Content".to_string());
        let style = CssStyle::browser_default();
        let items = [&first, &second];
        let styles = [style.clone(), style.clone()];

        let mut fixed = style.clone();
        fixed.display = Display::Grid;
        fixed.grid_template_columns =
            vec![CssGridTrack::Length(100.0), CssGridTrack::Fraction(1.0)];
        fixed.gap = 12.0;
        assert_eq!(
            measured_widths(&items, &styles, 280.0, &fixed),
            [100.0, 168.0]
        );

        fixed.grid_template_columns = vec![CssGridTrack::MaxContent, CssGridTrack::Fraction(1.0)];
        let measured = measured_widths(&items, &styles, 280.0, &fixed);
        assert!(measured[0] < measured[1]);
    }

    #[test]
    fn grid_measurements_respect_minmax_minimums_and_fraction_growth() {
        let first = HtmlDocumentNode::Text("First".to_string());
        let second = HtmlDocumentNode::Text("Second".to_string());
        let style = CssStyle::browser_default();
        let items = [&first, &second];
        let styles = [style.clone(), style.clone()];
        let mut grid = style;
        grid.display = Display::Grid;
        grid.gap = 10.0;
        grid.grid_template_columns = vec![
            CssGridTrack::MinMax {
                min: CssGridTrackBreadth::Length(80.0),
                max: CssGridTrackBreadth::Fraction(1.0),
            },
            CssGridTrack::MinMax {
                min: CssGridTrackBreadth::Length(120.0),
                max: CssGridTrackBreadth::Fraction(1.0),
            },
        ];

        assert_eq!(
            measured_widths(&items, &styles, 310.0, &grid),
            [150.0, 150.0]
        );
    }

    #[test]
    fn grid_measurements_cover_percent_min_content_auto_and_empty_tracks() {
        let first = HtmlDocumentNode::Text("long word".to_string());
        let second = HtmlDocumentNode::Text("content".to_string());
        let style = CssStyle::browser_default();
        let items = [&first, &second];
        let styles = [style.clone(), style.clone()];
        let mut grid = style;
        grid.display = Display::Grid;
        grid.grid_template_columns = vec![CssGridTrack::Percent(0.25), CssGridTrack::MinContent];
        let intrinsic = measured_widths(&items, &styles, 200.0, &grid);
        assert_eq!(intrinsic[0], 50.0);
        assert!(intrinsic[1] > 40.0);

        grid.grid_template_columns = vec![CssGridTrack::Length(50.0), CssGridTrack::Auto];
        assert_eq!(
            measured_widths(&items, &styles, 200.0, &grid),
            [50.0, 150.0]
        );
        grid.grid_template_columns = vec![CssGridTrack::Length(50.0)];
        assert_eq!(
            measured_widths(&items[..1], &styles[..1], 200.0, &grid),
            [50.0]
        );
        grid.grid_template_columns.clear();
        assert_eq!(
            measured_widths(&items[..1], &styles[..1], 200.0, &grid),
            [200.0]
        );
    }

    fn test_error(error: taffy::TaffyError) -> String {
        error.to_string()
    }

    #[test]
    fn test_error_preserves_real_taffy_child_failure() -> Result<(), String> {
        let mut tree: taffy::TaffyTree<()> = taffy::TaffyTree::new();
        let parent = tree.new_leaf(taffy::Style::default()).map_err(test_error)?;
        let error = tree
            .child_at_index(parent, 0)
            .err()
            .ok_or("child failure missing")?;
        assert!(matches!(
            &error,
            taffy::TaffyError::ChildIndexOutOfBounds {
                child_index: 0,
                child_count: 0,
                ..
            }
        ));
        assert_eq!(test_error(error.clone()), error.to_string());
        assert_eq!(
            projected_width("<div>no paragraph</div>").err(),
            Some("parsed section must be projected".to_string())
        );
        Ok(())
    }
}
