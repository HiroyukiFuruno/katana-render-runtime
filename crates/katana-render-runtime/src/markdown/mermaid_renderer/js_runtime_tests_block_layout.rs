use super::*;
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use resvg::usvg::roxmltree::Node as SvgNode;
#[path = "js_runtime_tests_block_layout_bounds.rs"]
mod bounds;
pub(super) use bounds::assert_block_containment;
pub(super) const DOM_ADVANCE_UNITS_PER_PX: f64 = 64.0;

pub(super) fn semantic_node<'a, 'input>(
    root: SvgNode<'a, 'input>,
    suffix: &str,
) -> Result<SvgNode<'a, 'input>, String> {
    let mut found = root.descendants().filter(|item| {
        item.is_element()
            && item.tag_name().name() == "g"
            && item
                .attribute("class")
                .is_some_and(|class| class.split_whitespace().any(|part| part == "node"))
            && item.attribute("id").is_some_and(|id| id.ends_with(suffix))
    });
    let node = found
        .next()
        .ok_or_else(|| format!("missing Block node {suffix:?}"))?;
    if found.next().is_some() {
        return Err(format!("duplicate Block node {suffix:?}"));
    }
    Ok(node)
}

pub(super) fn label_rectangle<'a, 'input>(
    group: SvgNode<'a, 'input>,
) -> Result<SvgNode<'a, 'input>, String> {
    let mut found = group.descendants().filter(|item| {
        item.is_element()
            && item.tag_name().name() == "rect"
            && item.attribute("class").is_some_and(|class| {
                class
                    .split_whitespace()
                    .any(|part| part == "label-container")
            })
    });
    let rect = found
        .next()
        .ok_or_else(|| "missing label-container rectangle".to_owned())?;
    if found.next().is_some() {
        return Err("duplicate label-container rectangle".to_owned());
    }
    Ok(rect)
}

pub(super) fn node_text(group: SvgNode<'_, '_>) -> String {
    group
        .descendants()
        .filter(|item| item.is_text())
        .filter_map(|item| item.text())
        .collect()
}

pub(super) fn assert_block_label_width(
    root: SvgNode<'_, '_>,
    label_rect: SvgNode<'_, '_>,
) -> Result<(), String> {
    let advance = native_html_text_advance(root, "A wide one in the middle")?;
    let actual = svg_number(label_rect, "width")?;
    /* WHY: DOM配置幅の1/64px切り上げ後に、Blockの左右16px paddingを加える。 */
    let expected = (f64::from(advance) * DOM_ADVANCE_UNITS_PER_PX).ceil()
        / DOM_ADVANCE_UNITS_PER_PX
        + f64::from(BLOCK_LABEL_HORIZONTAL_PADDING_PX);
    if actual != expected {
        return Err(format!(
            "Block label width {actual} differs from DOM width plus padding {expected} (raw advance {advance})"
        ));
    }
    Ok(())
}

fn native_html_text_advance(root: SvgNode<'_, '_>, text: &str) -> Result<f32, String> {
    let (family, size) = stylesheet_font(root)?;
    SvgRasterizeOps::with_html_font_render_scope(|| {
        SvgRasterizeOps::measure_html_text(
            text,
            family,
            size,
            BLOCK_NORMAL_FONT_WEIGHT,
            false,
            0.0,
            None,
        )
    })
    .map_err(|error| error.to_string())
}

#[test]
fn native_block_label_measurement_propagates_svg_parse_errors()
-> Result<(), Box<dyn std::error::Error>> {
    let document = resvg::usvg::roxmltree::Document::parse(
        "<svg><style>font-family:;font-size:16px;</style></svg>",
    )?;
    let root = document.root_element();
    let error = native_html_text_advance(root, "\0")
        .err()
        .ok_or("invalid XML text unexpectedly measured")?;
    assert!(error.starts_with("Failed to parse SVG:"), "{error}");
    Ok(())
}

pub(super) fn stylesheet_font<'a, 'input>(
    root: SvgNode<'a, 'input>,
) -> Result<(&'a str, f32), String> {
    let stylesheet = root
        .descendants()
        .find(|item| item.is_element() && item.tag_name().name() == "style")
        .and_then(|item| item.text())
        .ok_or_else(|| "missing diagram stylesheet".to_owned())?;
    let family = css_value(stylesheet, "font-family:", ";")?;
    let size = css_value(stylesheet, "font-size:", "px;")?
        .parse::<f32>()
        .map_err(|error| error.to_string())?;
    Ok((family, size))
}

pub(super) fn css_value<'a>(stylesheet: &'a str, key: &str, end: &str) -> Result<&'a str, String> {
    stylesheet
        .split_once(key)
        .and_then(|(_, value)| value.split_once(end).map(|(value, _)| value.trim()))
        .ok_or_else(|| format!("missing stylesheet value {key:?}"))
}

pub(super) fn svg_number(node: SvgNode<'_, '_>, name: &str) -> Result<f64, String> {
    node.attribute(name)
        .ok_or_else(|| format!("missing SVG attribute {name:?}"))?
        .parse::<f64>()
        .map_err(|error| error.to_string())
}

pub(super) fn assert_label_style(label_rect: SvgNode<'_, '_>) -> Result<(), String> {
    let style = label_rect
        .attribute("style")
        .map_or("", std::convert::identity);
    for declaration in ["fill:#969", "stroke:#333", "stroke-width:4px"] {
        if !style.contains(declaration) {
            return Err(format!("missing label style {declaration:?}"));
        }
    }
    Ok(())
}
