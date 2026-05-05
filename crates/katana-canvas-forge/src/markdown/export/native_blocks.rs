use crate::markdown::MarkdownError;
use std::ops::Range;

const SVG_VIEWBOX_PARTS: usize = 4;
const VIEWBOX_MIN_DIMENSION: f32 = 0.0;
const SVG_DIMENSION_SCALE_DEFAULT: u32 = 320;
const SVG_DIMENSION_SCALE_MAX: f32 = 8192.0;
const MIN_SVG_DIMENSION_INDEX: usize = 2;
const MAX_SVG_DIMENSION_INDEX: usize = 3;

#[derive(Clone)]
pub(crate) enum NativeDocumentBlock {
    Text(super::native_text::NativeTextLine),
    Svg(NativeSvgBlock),
}

#[derive(Clone)]
pub(crate) struct NativeSvgBlock {
    pub(crate) svg: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

pub(crate) struct NativeDocumentBlocks;

impl NativeDocumentBlocks {
    pub(crate) fn parse(
        html: &str,
        is_dark: bool,
    ) -> Result<Vec<NativeDocumentBlock>, MarkdownError> {
        let mut blocks = Vec::new();
        let mut cursor = 0;
        for range in NativeSvgRanges::find(html) {
            Self::push_text(&mut blocks, &html[cursor..range.start], is_dark)?;
            blocks.push(NativeDocumentBlock::Svg(NativeSvgBlock::parse(
                &html[range.clone()],
            )?));
            cursor = range.end;
        }
        Self::push_text(&mut blocks, &html[cursor..], is_dark)?;
        Ok(blocks)
    }

    fn push_text(
        blocks: &mut Vec<NativeDocumentBlock>,
        html: &str,
        is_dark: bool,
    ) -> Result<(), MarkdownError> {
        for line in super::native_text::extract_lines(html, is_dark)? {
            blocks.push(NativeDocumentBlock::Text(line));
        }
        Ok(())
    }
}

struct NativeSvgRanges;

impl NativeSvgRanges {
    fn find(html: &str) -> Vec<Range<usize>> {
        let lower_html = html.to_ascii_lowercase();
        let mut ranges = Vec::new();
        let mut cursor = 0;
        while let Some(start) = Self::find_open_tag(&lower_html, cursor) {
            let Some(end) = Self::find_matching_close_tag(&lower_html, start) else {
                break;
            };
            ranges.push(start..end);
            cursor = end;
        }
        ranges
    }

    fn find_matching_close_tag(lower_html: &str, start: usize) -> Option<usize> {
        let mut depth = 0;
        let mut cursor = start;
        loop {
            let open = Self::find_open_tag(lower_html, cursor);
            let close = lower_html[cursor..].find("</svg>").map(|it| cursor + it);
            match (open, close) {
                (Some(open), Some(close)) if open < close => {
                    depth += 1;
                    cursor = open + "<svg".len();
                }
                (_, Some(close)) if depth > 1 => {
                    depth -= 1;
                    cursor = close + "</svg>".len();
                }
                (_, Some(close)) => return Some(close + "</svg>".len()),
                _ => return None,
            }
        }
    }

    fn find_open_tag(lower_html: &str, cursor: usize) -> Option<usize> {
        let mut search = cursor;
        while let Some(found) = lower_html[search..].find("<svg") {
            let start = search + found;
            if Self::is_tag_boundary(lower_html, start + "<svg".len()) {
                return Some(start);
            }
            search = start + "<svg".len();
        }
        None
    }

    fn is_tag_boundary(lower_html: &str, index: usize) -> bool {
        lower_html
            .as_bytes()
            .get(index)
            .is_none_or(|it| it.is_ascii_whitespace() || matches!(it, b'>' | b'/'))
    }
}

impl NativeSvgBlock {
    fn parse(svg: &str) -> Result<Self, MarkdownError> {
        let (width, height) = NativeSvgSize::parse(svg);
        let svg = super::native_svg_root::NativeSvgRoot::with_numeric_size(svg, width, height)?;
        Ok(Self { svg, width, height })
    }

    pub(crate) fn scale_for(&self, max_width: u32) -> f32 {
        (max_width as f32 / self.width.max(1) as f32).min(1.0)
    }
}

struct NativeSvgSize;

impl NativeSvgSize {
    fn parse(svg: &str) -> (u32, u32) {
        if let Some(size) = Self::from_view_box(svg) {
            return size;
        }
        (
            Self::dimension(svg, "width"),
            Self::dimension(svg, "height"),
        )
    }

    fn from_view_box(svg: &str) -> Option<(u32, u32)> {
        let view_box = svg_attribute(svg, "viewBox")?;
        let values = view_box
            .split(|it: char| it.is_whitespace() || it == ',')
            .filter(|it| !it.is_empty())
            .map(str::parse::<f32>)
            .collect::<Result<Vec<_>, _>>()
            .ok();
        values.and_then(|it| Self::view_box_size(&it))
    }

    fn view_box_size(values: &[f32]) -> Option<(u32, u32)> {
        if values.len() != SVG_VIEWBOX_PARTS
            || values[MIN_SVG_DIMENSION_INDEX] <= VIEWBOX_MIN_DIMENSION
            || values[MAX_SVG_DIMENSION_INDEX] <= VIEWBOX_MIN_DIMENSION
        {
            return None;
        }
        Some((
            ceil_dimension(values[MIN_SVG_DIMENSION_INDEX]),
            ceil_dimension(values[MAX_SVG_DIMENSION_INDEX]),
        ))
    }

    fn dimension(svg: &str, name: &str) -> u32 {
        svg_attribute(svg, name)
            .map(|value| value.trim_end_matches("px"))
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|value| *value > 0.0)
            .map(ceil_dimension)
            .unwrap_or(SVG_DIMENSION_SCALE_DEFAULT)
    }
}

fn ceil_dimension(value: f32) -> u32 {
    value.ceil().clamp(1.0, SVG_DIMENSION_SCALE_MAX) as u32
}

fn svg_attribute<'a>(svg: &'a str, name: &str) -> Option<&'a str> {
    let marker = format!(r#"{name}=""#);
    let start = svg.find(&marker)? + marker.len();
    let end = svg[start..].find('"')?;
    Some(&svg[start..start + end])
}

#[cfg(test)]
#[path = "native_blocks_tests.rs"]
mod tests;
