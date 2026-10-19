use super::constants::MIN_LAYOUT_WIDTH;
use super::document::attribute;
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::svg::escape_xml;
use super::types::LayoutContext;
use crate::renderer::backends::html_document::{
    EMBEDDED_SVG_HEIGHT_PLACEHOLDER, EMBEDDED_SVG_MARKUP_ATTRIBUTE, EMBEDDED_SVG_WIDTH_PLACEHOLDER,
    EMBEDDED_SVG_X_PLACEHOLDER, EMBEDDED_SVG_Y_PLACEHOLDER,
};

#[path = "layout_media_png.rs"]
mod png;

#[path = "layout_media_svg_size.rs"]
mod svg_size;

use svg_size::{embedded_svg_size, intrinsic_svg_width};
#[cfg(test)]
use svg_size::{parse_svg_length, parse_view_box};

const DEFAULT_IMAGE_MAX_HEIGHT: f32 = 240.0;

impl HtmlLayoutRenderer {
    pub(super) fn render_image(
        &mut self,
        attributes: &[(String, String)],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        let start = y + style.margin_top;
        let x = x + style.margin_left - self.scroll_x;
        let available_width =
            (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
        let Some(source) = attribute(attributes, "src") else {
            return start + style.margin_bottom;
        };
        let (image_width, image_height) = image_box_size(source, available_width, style);
        let image_y = start - self.scroll_y;
        self.svg.push_str(&format!(
            r#"<image href="{}" x="{x}" y="{image_y}" width="{image_width}" height="{image_height}" preserveAspectRatio="xMidYMid meet"/>"#,
            escape_xml(source)
        ));
        start + image_height + style.margin_bottom
    }

    pub(super) fn render_embedded_svg(
        &mut self,
        attributes: &[(String, String)],
        layout: LayoutContext<'_>,
    ) -> f32 {
        let start = layout.y + layout.style.margin_top;
        let available_width = (layout.width - layout.style.margin_left - layout.style.margin_right)
            .max(MIN_LAYOUT_WIDTH);
        let Some(markup) = attribute(attributes, EMBEDDED_SVG_MARKUP_ATTRIBUTE) else {
            return start + layout.style.margin_bottom;
        };
        let (svg_width, svg_height) = embedded_svg_size(attributes, available_width, layout.style);
        /* WHY: 自然寸法と幅制約を適用したSVGの実幅からauto marginを解決する。 */
        let x = embedded_svg_x(layout.x, available_width, svg_width, layout.style) - self.scroll_x;
        let svg = position_embedded_svg(markup, x, start - self.scroll_y, svg_width, svg_height);
        self.svg.push_str(&svg);
        start + svg_height + layout.style.margin_bottom
    }
}

fn embedded_svg_x(x: f32, available_width: f32, svg_width: f32, style: &CssStyle) -> f32 {
    let auto_margin_count =
        usize::from(style.margin_left_auto) + usize::from(style.margin_right_auto);
    let remaining_width = (available_width - svg_width).max(0.0);
    let auto_left = if style.margin_left_auto {
        remaining_width / auto_margin_count as f32
    } else {
        0.0
    };
    x + style.margin_left + auto_left
}

pub(super) fn intrinsic_image_width(
    node: &crate::renderer::backends::html_document::HtmlDocumentNode,
    available_width: f32,
    style: &CssStyle,
) -> Option<f32> {
    let crate::renderer::backends::html_document::HtmlDocumentNode::Element {
        tag, attributes, ..
    } = node
    else {
        return None;
    };
    match tag.as_str() {
        "img" => attribute(attributes, "src")
            .map(|source| image_box_size(source, available_width, style).0),
        "svg" => intrinsic_svg_width(attributes, available_width, style),
        _ => None,
    }
}

pub(super) fn resolved_svg_width(
    node: &crate::renderer::backends::html_document::HtmlDocumentNode,
    available_width: f32,
    style: &CssStyle,
) -> Option<f32> {
    let crate::renderer::backends::html_document::HtmlDocumentNode::Element {
        tag, attributes, ..
    } = node
    else {
        return None;
    };
    (tag == "svg").then(|| embedded_svg_size(attributes, available_width, style).0)
}

fn image_box_size(source: &str, available_width: f32, style: &CssStyle) -> (f32, f32) {
    let natural = png::data_dimensions(source).unwrap_or((
        available_width,
        available_width.min(DEFAULT_IMAGE_MAX_HEIGHT),
    ));
    let mut width = style
        .width
        .map(|width| width.resolve(available_width))
        .unwrap_or(natural.0);
    let mut height = style
        .height
        .unwrap_or_else(|| natural.1 * width / natural.0);
    let maximum_width = style
        .max_width
        .map(|maximum| maximum.resolve(available_width))
        .unwrap_or(available_width)
        .min(available_width);
    scale_image_to_maximum(&mut width, &mut height, maximum_width, f32::INFINITY);
    if let Some(maximum_height) = style.max_height {
        scale_image_to_maximum(&mut width, &mut height, f32::INFINITY, maximum_height);
    }
    (width.max(MIN_LAYOUT_WIDTH), height.max(MIN_LAYOUT_WIDTH))
}

fn scale_image_to_maximum(
    width: &mut f32,
    height: &mut f32,
    maximum_width: f32,
    maximum_height: f32,
) {
    let scale = (maximum_width / *width)
        .min(maximum_height / *height)
        .min(1.0);
    *width *= scale;
    *height *= scale;
}

fn position_embedded_svg(markup: &str, x: f32, y: f32, width: f32, height: f32) -> String {
    markup
        .replace(EMBEDDED_SVG_X_PLACEHOLDER, &x.to_string())
        .replace(EMBEDDED_SVG_Y_PLACEHOLDER, &y.to_string())
        .replace(EMBEDDED_SVG_WIDTH_PLACEHOLDER, &width.to_string())
        .replace(EMBEDDED_SVG_HEIGHT_PLACEHOLDER, &height.to_string())
}

#[cfg(test)]
#[path = "layout_media_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "layout_media_svg_intrinsic_tests.rs"]
mod svg_intrinsic_tests;
