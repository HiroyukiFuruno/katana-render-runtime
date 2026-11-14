#[cfg(test)]
use super::constants::DEFAULT_FONT_SIZE;
use std::borrow::Cow;

#[path = "style_construction.rs"]
mod construction;

#[path = "style_border_radius.rs"]
mod border_radius;
#[path = "style_columns_declaration.rs"]
mod columns_declaration;
#[path = "style_declaration.rs"]
mod declaration;
#[path = "style_declaration_element.rs"]
mod declaration_element;
#[path = "style_dimension_declaration.rs"]
mod dimension_declaration;
#[cfg(test)]
#[path = "style_property_case_tests.rs"]
mod property_case_tests;
#[path = "style_rotation.rs"]
mod rotation;
#[path = "style_border_declaration.rs"]
mod style_border_declaration;
#[path = "style_box.rs"]
mod style_box;
#[path = "style_box_declaration.rs"]
mod style_box_declaration;
#[path = "style_flex_declaration.rs"]
mod style_flex_declaration;
#[path = "style_shadow_declaration.rs"]
mod style_shadow_declaration;
#[path = "style_typography_declaration.rs"]
mod style_typography_declaration;
#[path = "style_tag_defaults.rs"]
mod tag_defaults;
#[cfg(test)]
#[path = "style_text_defaults_tests.rs"]
mod text_defaults_tests;
#[path = "style_text_metrics.rs"]
mod text_metrics;
#[path = "style_text_spacing_trim.rs"]
mod text_spacing_trim;
#[path = "style_text_transform.rs"]
mod text_transform;
#[path = "style_types.rs"]
mod types;
#[path = "style_value.rs"]
mod value;

pub(super) use text_spacing_trim::CssTextSpacingTrim;
pub(super) use types::{
    CssBorderStyle, CssBoxShadow, CssBoxSizing, CssFloat, CssGridTrack, CssGridTrackBreadth,
    CssLength, CssOverflow, CssOverflowAxis, CssPosition, CssStyle, CssTextAlign, CssTextTransform,
    CssVerticalAlign, CssWhiteSpace,
};

pub(super) fn normalized_property_name(name: &str) -> Cow<'_, str> {
    if name.bytes().any(|byte| byte.is_ascii_uppercase()) {
        Cow::Owned(name.to_ascii_lowercase())
    } else {
        Cow::Borrowed(name)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn paragraph_default_margins_are_one_em_and_author_reset_wins() {
        let mut inherited = CssStyle::browser_default();
        inherited.font_size = 20.0;
        let style = CssStyle::from_element("p", &[], &inherited);
        assert_eq!((style.margin_top, style.margin_bottom), (20.0, 20.0));
        let style = CssStyle::from_element(
            "p",
            &[("style".to_string(), "margin:0".to_string())],
            &inherited,
        );
        assert_eq!((style.margin_top, style.margin_bottom), (0.0, 0.0));
    }

    #[test]
    fn code_user_agent_font_applies_before_author_typography() {
        let inherited = CssStyle::browser_default();
        let default_style = CssStyle::from_element("code", &[], &inherited);
        assert_eq!(default_style.font_family, "monospace");
        let authored = CssStyle::from_element(
            "code",
            &[("style".to_string(), "font-family: Helvetica".to_string())],
            &inherited,
        );
        assert_eq!(authored.font_family, "Helvetica");
        let boxed = CssStyle::from_element(
            "code",
            &[(
                "style".to_string(),
                "background:#eee;padding:2px".to_string(),
            )],
            &inherited,
        );
        assert_eq!(boxed.font_family, "monospace");
    }
    #[test]
    fn normal_line_height_uses_font_metrics_and_preserves_explicit_values() {
        use crate::renderer::backends::html_document::HtmlDocumentNode;
        let children = vec![HtmlDocumentNode::Text("日本語".to_string())];
        let mut style = CssStyle::browser_default();
        style.font_family = "Hiragino Sans, sans-serif".to_string();
        style.resolve_text_line_height(&children);
        assert!(style.line_height >= style.font_size);
        style.apply_line_height("1.65");
        style.resolve_text_line_height(&children);
        assert_eq!(style.line_height, 26.390625);
        style.apply_line_height("normal");
        assert!(style.normal_line_height);
    }
    use super::super::constants::LINE_HEIGHT_FACTOR;
    use super::{
        CssBoxSizing, CssGridTrack, CssLength, CssOverflow, CssStyle, CssTextAlign,
        CssVerticalAlign, DEFAULT_FONT_SIZE,
    };

    fn parsed_flow_style() -> CssStyle {
        CssStyle::from_attributes(
            &[(
                "style".to_string(),
                "display:grid;gap:12px;grid-template-columns:repeat(3,1fr);flex-direction:column;flex-wrap:wrap;flex-grow:2;flex-shrink:0;align-items:center;justify-content:space-between".to_string(),
            )],
            &CssStyle::browser_default(),
        )
    }

    fn parsed_flex_style(value: &str) -> CssStyle {
        CssStyle::from_attributes(
            &[("style".to_string(), format!("flex:{value}"))],
            &CssStyle::browser_default(),
        )
    }

    #[test]
    fn box_shorthands_expand_and_longhands_override_individual_edges() {
        let attributes = vec![(
            "style".to_string(),
            "padding: 1px 2px 3px 4px; margin: -5px 6px 7px; padding-left: 9px; margin-right: 8px"
                .to_string(),
        )];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());

        assert_eq!(
            [
                style.padding_top,
                style.padding_right,
                style.padding_bottom,
                style.padding_left,
            ],
            [1.0, 2.0, 3.0, 9.0]
        );
        assert_eq!(
            [
                style.margin_top,
                style.margin_right,
                style.margin_bottom,
                style.margin_left,
            ],
            [-5.0, 8.0, 7.0, 6.0]
        );
    }

    #[test]
    fn invalid_box_shorthand_does_not_erase_existing_edges() {
        let attributes = vec![(
            "style".to_string(),
            "margin-top: 3px; margin-bottom: 4px; margin: var(--missing)".to_string(),
        )];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());

        assert_eq!(style.margin_top, 3.0);
        assert_eq!(style.margin_bottom, 4.0);
    }

    #[test]
    fn flow_properties_parse_into_typed_layout_values() {
        let style = parsed_flow_style();

        assert_eq!(style.display, taffy::style::Display::Grid);
        assert_eq!(style.gap, 12.0);
        assert_eq!(style.row_gap, Some(12.0));
        assert_eq!(style.flow_column_gap, Some(12.0));
        assert_eq!(
            style.grid_template_columns,
            vec![CssGridTrack::Fraction(1.0); 3]
        );
        assert_eq!(style.flex_direction, taffy::style::FlexDirection::Column);
        assert_eq!(style.flex_wrap, taffy::style::FlexWrap::Wrap);
    }

    #[test]
    fn gap_shorthand_expands_two_values_and_longhands_follow_source_order() {
        let style = CssStyle::from_attributes(
            &[(
                "style".to_string(),
                "gap: 6px 20px; row-gap: 8px; column-gap: 24px".to_string(),
            )],
            &CssStyle::browser_default(),
        );
        assert_eq!(style.gap, 6.0);
        assert_eq!(style.row_gap, Some(8.0));
        assert_eq!(style.flow_column_gap, Some(24.0));
        assert_eq!(style.column_gap, 24.0);
    }

    #[test]
    fn invalid_gap_values_do_not_erase_valid_values() {
        let style = CssStyle::from_attributes(
            &[(
                "style".to_string(),
                "gap: 6px 20px; gap: -1px; gap: 1px 2px 3px; row-gap: nope".to_string(),
            )],
            &CssStyle::browser_default(),
        );
        assert_eq!(style.gap, 6.0);
        assert_eq!(style.row_gap, Some(6.0));
        assert_eq!(style.flow_column_gap, Some(20.0));
    }

    #[test]
    fn normal_multicolumn_gap_is_one_em_while_flow_gap_is_zero() {
        let style = CssStyle::from_attributes(
            &[(
                "style".to_string(),
                "display:grid;font-size:20px".to_string(),
            )],
            &CssStyle::browser_default(),
        );
        assert_eq!(style.column_gap, 20.0);
        assert_eq!(style.row_gap, None);
        assert_eq!(style.flow_column_gap, None);
        assert_eq!(style.gap, 0.0);
    }

    #[test]
    fn normal_gap_values_use_zero_for_flow_and_one_em_for_multicolumn() {
        let style = CssStyle::from_attributes(
            &[(
                "style".to_string(),
                "display:flex;font-size:20px;gap:normal;column-gap:normal".to_string(),
            )],
            &CssStyle::browser_default(),
        );
        assert_eq!(style.gap, 0.0);
        assert_eq!(style.row_gap, Some(0.0));
        assert_eq!(style.flow_column_gap, Some(0.0));
        assert_eq!(style.column_gap, 20.0);
    }

    #[test]
    fn flow_alignment_and_flex_factors_parse_into_typed_values() {
        let style = parsed_flow_style();

        assert_eq!(style.flex_grow, 2.0);
        assert_eq!(style.flex_shrink, 0.0);
        assert_eq!(style.flex_basis, None);
        assert_eq!(style.align_items, Some(taffy::style::AlignItems::CENTER));
        assert_eq!(
            style.justify_content,
            Some(taffy::style::JustifyContent::SPACE_BETWEEN)
        );
    }

    #[test]
    fn flex_shorthand_sets_browser_compatible_grow_shrink_and_basis() {
        let one = parsed_flex_style("1");
        assert_eq!(one.flex_grow, 1.0);
        assert_eq!(one.flex_shrink, 1.0);
        assert_eq!(one.flex_basis, Some(CssLength::Percent(0.0)));

        let explicit = parsed_flex_style("2 0 25%");
        assert_eq!(explicit.flex_grow, 2.0);
        assert_eq!(explicit.flex_shrink, 0.0);
        assert_eq!(explicit.flex_basis, Some(CssLength::Percent(0.25)));
    }

    #[test]
    fn flex_shorthand_keywords_and_explicit_basis_use_browser_defaults() {
        let auto = parsed_flex_style("auto");
        assert_eq!(
            (auto.flex_grow, auto.flex_shrink, auto.flex_basis),
            (1.0, 1.0, None)
        );
        let none = parsed_flex_style("none");
        assert_eq!(
            (none.flex_grow, none.flex_shrink, none.flex_basis),
            (0.0, 0.0, None)
        );
        let initial = parsed_flex_style("initial");
        assert_eq!(
            (initial.flex_grow, initial.flex_shrink, initial.flex_basis),
            (0.0, 1.0, None)
        );

        let basis = CssStyle::from_attributes(
            &[("style".to_string(), "flex-basis:40px".to_string())],
            &CssStyle::browser_default(),
        );
        assert_eq!(basis.flex_basis, Some(CssLength::Px(40.0)));
    }

    #[test]
    fn percentage_width_and_max_width_resolve_against_available_space() {
        let attributes = vec![(
            "style".to_string(),
            "width: 75%; max-width: 200px".to_string(),
        )];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());

        assert_eq!(style.explicit_width(400.0), Some(200.0));
        assert_eq!(style.explicit_width(200.0), Some(150.0));

        let max_only = vec![("style".to_string(), "max-width: 120px".to_string())];
        let max_only = CssStyle::from_attributes(&max_only, &CssStyle::browser_default());
        assert_eq!(max_only.box_width(400.0), 120.0);
    }

    #[test]
    fn assigned_flow_width_replaces_pre_layout_width_constraints() {
        let mut assigned_outer = CssStyle::from_attributes(
            &[(
                "style".to_string(),
                "box-sizing: content-box; min-width: 60px; max-width: 80px; padding: 10px"
                    .to_string(),
            )],
            &CssStyle::browser_default(),
        );
        assigned_outer.assign_outer_width(120.0);
        assert_eq!(assigned_outer.box_width(200.0), 120.0);
        assert_eq!(assigned_outer.width, Some(CssLength::Px(100.0)));
        assert_eq!(assigned_outer.min_width, None);
        assert_eq!(assigned_outer.max_width, None);
    }

    #[test]
    fn relative_font_and_box_lengths_resolve_without_unitless_line_height_bug() {
        let attributes = vec![(
            "style".to_string(),
            "font-size: 1.5rem; line-height: 1.25; padding: 0.5em; margin-top: -1rem".to_string(),
        )];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());

        assert_eq!(style.font_size, 24.0);
        assert_eq!(style.line_height, 30.0);
        assert_eq!(style.padding_top, 12.0);
        assert_eq!(style.margin_top, -16.0);
    }

    #[test]
    fn heading_em_font_size_uses_parent_size_instead_of_user_agent_heading_size() {
        let mut parent = CssStyle::browser_default();
        parent.font_size = 14.0;
        let style = CssStyle::from_element(
            "h1",
            &[("style".to_string(), "font-size: 1.45em".to_string())],
            &parent,
        );
        assert!((style.font_size - 20.3).abs() < 0.01, "{}", style.font_size);
    }

    #[test]
    fn heading_defaults_use_chromium_em_metrics_and_author_overrides() {
        let mut inherited = CssStyle::browser_default();
        inherited.font_size = 20.0;
        let expected = [
            ("h1", 40.0, 26.8),
            ("h2", 30.0, 24.9),
            ("h3", 23.4, 23.4),
            ("h4", 20.0, 26.6),
            ("h5", 16.6, 27.722),
            ("h6", 13.4, 31.222),
        ];
        for (tag, font_size, margin) in expected {
            let style = CssStyle::from_element(tag, &[], &inherited);
            assert!((style.font_size - font_size).abs() < 0.001, "{tag}");
            assert!((style.margin_top - margin).abs() < 0.001, "{tag}");
        }
        let authored = CssStyle::from_element(
            "h1",
            &[("style".to_string(), "font-size:24px;margin:0".to_string())],
            &inherited,
        );
        assert_eq!(authored.font_size, 24.0);
        assert_eq!((authored.margin_top, authored.margin_bottom), (0.0, 0.0));
    }

    #[test]
    fn unitless_line_height_resolves_after_font_size_and_scales_when_inherited() {
        let before_font_size = vec![(
            "style".to_string(),
            "line-height: 1.5; font-size: 20px".to_string(),
        )];
        let parent = CssStyle::from_attributes(&before_font_size, &CssStyle::browser_default());
        assert_eq!(parent.line_height, 30.0);

        let child_attributes = vec![("style".to_string(), "font-size: 24px".to_string())];
        let child = CssStyle::from_attributes(&child_attributes, &parent);
        assert_eq!(child.line_height, 36.0);

        let fixed_parent = vec![(
            "style".to_string(),
            "line-height: 30px; font-size: 20px".to_string(),
        )];
        let fixed_parent = CssStyle::from_attributes(&fixed_parent, &CssStyle::browser_default());
        let fixed_child = CssStyle::from_attributes(&child_attributes, &fixed_parent);
        assert_eq!(fixed_child.line_height, 30.0);
    }

    #[test]
    fn invalid_line_height_does_not_erase_the_inherited_value() {
        let attributes = vec![("style".to_string(), "line-height: invalid".to_string())];
        let inherited = CssStyle::browser_default();
        let style = CssStyle::from_attributes(&attributes, &inherited);

        assert_eq!(style.line_height, inherited.line_height);
        assert_eq!(style.line_height_factor, inherited.line_height_factor);
    }

    #[test]
    fn monospace_families_use_their_advance_width_for_layout_measurement() {
        let proportional = CssStyle::browser_default();
        let monospace = CssStyle::from_attributes(
            &[(
                "style".to_string(),
                "font-family: IBM Plex Mono, monospace".to_string(),
            )],
            &proportional,
        );

        assert_eq!(proportional.text_character_width_factor(), 0.55);
        assert_eq!(monospace.text_character_width_factor(), 0.6);
    }

    #[test]
    fn hidden_attribute_maps_to_display_none() {
        let attributes = vec![("hidden".to_string(), String::new())];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());

        assert_eq!(style.display, taffy::style::Display::None);
    }

    #[test]
    fn padding_longhands_and_invalid_extensions_preserve_each_edge() {
        let attributes = vec![(
            "style".to_string(),
            "padding: 1px; padding-top: 2px; padding-right: 3px; padding-bottom: 4px; padding-inline: 9px; margin-inline: 9px; padding: 1px 2px 3px 4px 5px"
                .to_string(),
        )];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());

        assert_eq!(
            [
                style.padding_top,
                style.padding_right,
                style.padding_bottom,
                style.padding_left,
            ],
            [2.0, 3.0, 4.0, 1.0]
        );
    }

    #[test]
    fn alternate_relative_and_grid_values_resolve_without_fallbacks() {
        let attributes = vec![(
            "style".to_string(),
            "padding: 2px 3px; font-size: 125%; line-height: 150%; grid-template-columns: 1fr 2fr 1fr"
                .to_string(),
        )];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());
        assert_eq!(style.padding_top, 2.0);
        assert_eq!(style.padding_right, 3.0);
        assert_eq!(style.font_size, 20.0);
        assert_eq!(style.line_height, 30.0);
        assert_eq!(
            style.grid_template_columns,
            vec![
                CssGridTrack::Fraction(1.0),
                CssGridTrack::Fraction(2.0),
                CssGridTrack::Fraction(1.0),
            ]
        );

        let normal = vec![("style".to_string(), "line-height: normal".to_string())];
        let normal = CssStyle::from_attributes(&normal, &CssStyle::browser_default());
        assert_eq!(normal.line_height, DEFAULT_FONT_SIZE * LINE_HEIGHT_FACTOR);

        let em = vec![("style".to_string(), "line-height: 2em".to_string())];
        let em = CssStyle::from_attributes(&em, &CssStyle::browser_default());
        assert_eq!(em.line_height, DEFAULT_FONT_SIZE * 2.0);
    }

    #[test]
    fn grid_tracks_preserve_declared_sizes() {
        let attributes = vec![(
            "style".to_string(),
            "display:grid; grid-template-columns: 100px max-content 2fr 25%".to_string(),
        )];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());
        assert_eq!(
            style.grid_template_columns,
            vec![
                CssGridTrack::Length(100.0),
                CssGridTrack::MaxContent,
                CssGridTrack::Fraction(2.0),
                CssGridTrack::Percent(0.25),
            ]
        );
    }

    #[test]
    fn grid_repeat_expands_each_declared_track() {
        let repeat = vec![(
            "style".to_string(),
            "grid-template-columns: repeat(2, 80px 1fr)".to_string(),
        )];
        let repeat = CssStyle::from_attributes(&repeat, &CssStyle::browser_default());
        assert_eq!(
            repeat.grid_template_columns,
            vec![
                CssGridTrack::Length(80.0),
                CssGridTrack::Fraction(1.0),
                CssGridTrack::Length(80.0),
                CssGridTrack::Fraction(1.0),
            ]
        );
    }

    #[test]
    fn invalid_grid_tracks_preserve_the_inherited_template() {
        let attributes = vec![(
            "style".to_string(),
            "grid-template-columns: minmax(broken)".to_string(),
        )];
        let inherited = CssStyle::browser_default();
        let style = CssStyle::from_attributes(&attributes, &inherited);

        assert_eq!(style.grid_template_columns, inherited.grid_template_columns);
    }

    #[test]
    fn typed_content_box_properties_preserve_browser_box_model() {
        let attributes = vec![(
            "style".to_string(),
            "width: 100px; padding: 10px; border: 2px solid #123456; border-radius: 6px; overflow: hidden"
                .to_string(),
        )];
        let content_box = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());

        assert_eq!(content_box.box_sizing, CssBoxSizing::ContentBox);
        assert_eq!(content_box.explicit_width(300.0), Some(124.0));
        assert_eq!(content_box.content_width(124.0), 100.0);
        assert_eq!(content_box.border_width, 2.0);
        assert_eq!(content_box.border_radius, CssLength::Px(6.0));
        assert_eq!(content_box.overflow, CssOverflow::Clip);
    }

    #[test]
    fn typed_typography_properties_preserve_browser_values() {
        let attributes = vec![(
            "style".to_string(),
            "font-family: Inter, sans-serif; font-style: italic; text-align: center; letter-spacing: 2px"
                .to_string(),
        )];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());

        assert_eq!(style.font_family, "Inter, sans-serif");
        assert!(style.italic);
        assert_eq!(style.text_align, CssTextAlign::Center);
        assert_eq!(style.letter_spacing, 2.0);
    }

    #[test]
    fn typed_border_box_dimensions_preserve_browser_box_model() {
        let attributes = vec![(
            "style".to_string(),
            "box-sizing: border-box; width: 100px; padding: 10px; border-width: 2px; border-style: solid".to_string(),
        )];
        let border_box = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());
        assert_eq!(border_box.box_sizing, CssBoxSizing::BorderBox);
        assert_eq!(border_box.explicit_width(300.0), Some(100.0));
        assert_eq!(border_box.content_width(100.0), 76.0);
    }

    #[test]
    fn vertical_align_uses_css_initial_and_html_table_ua_values() {
        let initial = CssStyle::browser_default();
        assert_eq!(initial.vertical_align, CssVerticalAlign::Baseline);

        let group = CssStyle::from_element("tbody", &[], &initial);
        assert_eq!(group.vertical_align, CssVerticalAlign::Middle);
        let row = CssStyle::from_element("tr", &[], &group);
        assert_eq!(row.vertical_align, CssVerticalAlign::Middle);
        let cell = CssStyle::from_element("td", &[], &row);
        assert_eq!(cell.vertical_align, CssVerticalAlign::Middle);
    }

    #[test]
    fn vertical_align_row_and_cell_follow_author_group_override() {
        let group = CssStyle::from_element(
            "tbody",
            &[("style".to_string(), "vertical-align: top".to_string())],
            &CssStyle::browser_default(),
        );
        let row = CssStyle::from_element("tr", &[], &group);
        let cell = CssStyle::from_element("td", &[], &row);
        assert_eq!(group.vertical_align, CssVerticalAlign::Top);
        assert_eq!(row.vertical_align, CssVerticalAlign::Top);
        assert_eq!(cell.vertical_align, CssVerticalAlign::Top);
    }

    #[test]
    fn generic_parent_keeps_css_initial_vertical_align() {
        let parent = CssStyle::browser_default();
        let child = CssStyle::from_element("div", &[], &parent);
        assert_eq!(parent.vertical_align, CssVerticalAlign::Baseline);
        assert_eq!(child.vertical_align, CssVerticalAlign::Baseline);
    }

    #[test]
    fn vertical_align_author_values_support_inherit_initial_and_invalid_preservation() {
        let group = CssStyle::from_element("tbody", &[], &CssStyle::browser_default());
        let row = CssStyle::from_element("tr", &[], &group);
        assert_eq!(group.vertical_align, CssVerticalAlign::Middle);
        assert_eq!(row.vertical_align, CssVerticalAlign::Middle);

        let inherit = CssStyle::from_attributes(
            &[("style".to_string(), "vertical-align: inherit".to_string())],
            &row,
        );
        assert_eq!(inherit.vertical_align, CssVerticalAlign::Middle);

        let initial = CssStyle::from_attributes(
            &[("style".to_string(), "vertical-align: initial".to_string())],
            &row,
        );
        assert_eq!(initial.vertical_align, CssVerticalAlign::Baseline);

        let invalid = CssStyle::from_attributes(
            &[(
                "style".to_string(),
                "vertical-align: top; vertical-align: unsupported".to_string(),
            )],
            &row,
        );
        assert_eq!(invalid.vertical_align, CssVerticalAlign::Top);
    }
}
