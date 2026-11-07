use super::MAX_RASTERIZED_SVG_EDGE;

#[path = "svg_rasterize_preprocess_geometry.rs"]
mod geometry;
#[path = "svg_rasterize_preprocess_marker.rs"]
mod marker;
#[path = "svg_rasterize_preprocess_shadow.rs"]
mod shadow;
#[path = "svg_rasterize_preprocess_source_edits.rs"]
mod source_edits;
#[path = "svg_rasterize_preprocess_style_prefix.rs"]
mod style_prefix;

const LIGHT_DARK_FUNCTION: &str = "light-dark(";
const FOREIGN_OBJECT_LOCAL_NAME: &str = "foreignObject";
const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";

pub(super) fn preprocess_for_rasterizer(svg_text: &str) -> String {
    let with_xml_entities = normalize_html_entities_for_xml(svg_text);
    let without_foreign_objects = strip_foreign_objects(&with_xml_entities);
    let with_normalized_styles =
        style_prefix::normalize_inline_style_prefixes(&without_foreign_objects)
            .unwrap_or(without_foreign_objects);
    let with_resolved_colors = resolve_light_dark_functions(&with_normalized_styles);
    let with_normalized_shadows = shadow::normalize_filter_shadow_colors(&with_resolved_colors);
    let with_normalized_markers = marker::normalize_marker_selectors(with_normalized_shadows);
    geometry::materialize_geometry_properties(with_normalized_markers)
}

pub(super) fn source_selector_matches(
    node: resvg::usvg::roxmltree::Node<'_, '_>,
    selector: &simplecss::Selector<'_>,
) -> bool {
    geometry::selector_matches_source_node(node, selector)
}

fn normalize_html_entities_for_xml(svg_text: &str) -> String {
    svg_text.replace("&nbsp;", "&#160;")
}

fn strip_foreign_objects(svg_text: &str) -> String {
    if !svg_text
        .as_bytes()
        .windows(FOREIGN_OBJECT_LOCAL_NAME.len())
        .any(|bytes| bytes.eq_ignore_ascii_case(FOREIGN_OBJECT_LOCAL_NAME.as_bytes()))
    {
        return svg_text.to_owned();
    }
    let Ok(document) = resvg::usvg::roxmltree::Document::parse(svg_text) else {
        return svg_text.to_owned();
    };
    let edits = document
        .descendants()
        .filter(|node| is_foreign_object(*node))
        .filter(|node| !node.ancestors().skip(1).any(is_foreign_object))
        .map(|node| source_edits::SourceEdit::new(node.range(), String::new()))
        .collect();
    source_edits::apply_source_edits(svg_text, edits, false)
}

fn is_foreign_object(node: resvg::usvg::roxmltree::Node<'_, '_>) -> bool {
    node.is_element()
        && node
            .tag_name()
            .name()
            .eq_ignore_ascii_case(FOREIGN_OBJECT_LOCAL_NAME)
        && node
            .tag_name()
            .namespace()
            .is_none_or(|namespace| namespace == SVG_NAMESPACE)
}

#[cfg(test)]
#[path = "svg_rasterize_preprocess_foreign_object_tests.rs"]
mod foreign_object_tests;

fn resolve_light_dark_functions(svg_text: &str) -> String {
    let mut result = String::with_capacity(svg_text.len());
    let mut remaining = svg_text;
    while let Some(start) = find_light_dark_function(remaining) {
        let content_start = start + LIGHT_DARK_FUNCTION.len();
        result.push_str(&remaining[..start]);
        let Some((content_end, light_color)) =
            parse_light_dark_function(&remaining[content_start..])
        else {
            result.push_str(&remaining[start..content_start]);
            remaining = &remaining[content_start..];
            continue;
        };
        result.push_str(light_color.trim());
        remaining = &remaining[content_start + content_end + 1..];
    }
    result.push_str(remaining);
    result
}

fn find_light_dark_function(text: &str) -> Option<usize> {
    text.to_ascii_lowercase().find(LIGHT_DARK_FUNCTION)
}

pub(super) fn parse_light_dark_function(content: &str) -> Option<(usize, &str)> {
    let mut depth = 0usize;
    let mut comma = None;
    for (index, character) in content.char_indices() {
        match character {
            '(' => depth += 1,
            ')' if depth == 0 => return comma.map(|comma_index| (index, &content[..comma_index])),
            ')' => depth -= 1,
            ',' if depth == 0 && comma.is_none() => comma = Some(index),
            _ => {}
        }
    }
    None
}

pub(super) fn effective_scale(width: f32, height: f32, requested_scale: f32) -> f32 {
    let positive_scale = requested_scale.max(f32::MIN_POSITIVE);
    let width_scale = MAX_RASTERIZED_SVG_EDGE / width.max(1.0);
    let height_scale = MAX_RASTERIZED_SVG_EDGE / height.max(1.0);
    positive_scale.min(width_scale).min(height_scale)
}

#[cfg(test)]
mod allocation_tests {
    use super::{resolve_light_dark_functions, shadow, strip_foreign_objects};

    #[test]
    fn unchanged_svg_preprocess_stages_preserve_content() {
        let source = "<svg><title>日本語</title></svg>";
        let stages: [fn(&str) -> String; 3] = [
            strip_foreign_objects,
            resolve_light_dark_functions,
            |source| shadow::normalize_filter_shadow_colors(source),
        ];
        for normalize in stages {
            let output = normalize(source);
            assert_eq!(output, source);
        }
    }

    #[test]
    fn light_dark_preprocess_preserves_malformed_calls_between_valid_calls() {
        let input = "日本語 LiGhT-DaRk(#111,#eee) light-dark(#123) LIGHT-DARK(#222,#fff)";
        assert_eq!(
            resolve_light_dark_functions(input),
            "日本語 #111 light-dark(#123) #222"
        );
    }
}
