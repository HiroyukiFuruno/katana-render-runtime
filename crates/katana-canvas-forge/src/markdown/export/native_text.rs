use crate::markdown::MarkdownError;

use super::native_text_parser as parser;

pub(super) const BODY_FONT_SIZE: u32 = 16;
pub(super) const CODE_FONT_SIZE: u32 = 14;
pub(super) const LINE_SPACING: u32 = 10;
pub(super) const TEXT_COLUMNS: usize = 82;
pub(super) const HEADING_FONT_SIZE_H1: u32 = 28;
pub(super) const HEADING_FONT_SIZE_H2: u32 = 22;
pub(super) const HEADING_FONT_SIZE_H3: u32 = 18;
pub(super) const HEADING_LEVEL_1: u8 = 1;
pub(super) const HEADING_LEVEL_2: u8 = 2;
pub(super) const HEADING_LEVEL_3: u8 = 3;
pub(super) const HEADING_LEVEL_4: u8 = 4;
pub(super) const HEADING_LEVEL_5: u8 = 5;
pub(super) const HEADING_LEVEL_6: u8 = 6;
pub(super) const HEADING_COLUMN_SIZE_H1: usize = 47;
pub(super) const HEADING_COLUMN_SIZE_H2: usize = 60;
pub(super) const HEADING_COLUMN_SIZE_H3: usize = 73;
pub(super) const WORD_SPACING: usize = 1;

/* Marker characters used to survive the tag-stripping pipeline */
pub(super) const HEADING_START_MARKER: &str = "\u{0001}";
pub(super) const HEADING_SEP_MARKER: &str = "\u{0002}";
pub(super) const HEADING_END_MARKER: &str = "\u{0003}";
pub(super) const CODE_FENCE_MARKER: &str = "\u{0004}";

const RED_LUMINANCE_WEIGHT: f32 = 0.299;
const GREEN_LUMINANCE_WEIGHT: f32 = 0.587;
const BLUE_LUMINANCE_WEIGHT: f32 = 0.114;
const DARK_THRESHOLD: f32 = 128.0;
const HEX_RADIX: u32 = 16;
const HEX_LONG_COLOR_LENGTH: usize = 6;
const HEX_SHORT_COLOR_LENGTH: usize = 3;
const RED_SHIFT_LONG: u32 = 16;
const GREEN_SHIFT_LONG: u32 = 8;
const SHORT_RED_SHIFT: u32 = 8;
const SHORT_GREEN_SHIFT: u32 = 4;
const SHORT_COLOR_MASK: u32 = 15;
const SHORT_COLOR_BITS: u8 = 4;
const COLOR_CHANNELS: usize = 3;

type ColorTriplet = [u8; COLOR_CHANNELS];

#[derive(Clone)]
pub(super) struct NativeTextLine {
    pub(super) text: String,
    pub(super) font_size: u32,
    pub(super) bold: bool,
    pub(super) is_code: bool,
    pub(super) spans: Vec<NativeTextSpan>,
}

#[derive(Clone)]
pub(super) struct NativeTextSpan {
    pub(super) text: String,
    pub(super) color: ColorTriplet,
}

impl NativeTextLine {
    pub(super) fn body(text: String) -> Self {
        Self {
            text,
            font_size: BODY_FONT_SIZE,
            bold: false,
            is_code: false,
            spans: vec![],
        }
    }

    pub(super) fn heading(text: String, level: u8) -> Self {
        let font_size = match level {
            HEADING_LEVEL_1 => HEADING_FONT_SIZE_H1,
            HEADING_LEVEL_2 => HEADING_FONT_SIZE_H2,
            HEADING_LEVEL_3 => HEADING_FONT_SIZE_H3,
            _ => BODY_FONT_SIZE,
        };

        Self {
            text,
            font_size,
            bold: true,
            is_code: false,
            spans: vec![],
        }
    }

    pub(super) fn code_highlighted(text: String, spans: Vec<NativeTextSpan>) -> Self {
        Self {
            text,
            font_size: CODE_FONT_SIZE,
            bold: false,
            is_code: true,
            spans,
        }
    }

    pub(super) fn line_height(&self) -> u32 {
        self.font_size + LINE_SPACING
    }

    pub(super) fn is_heading(&self) -> bool {
        self.bold && self.font_size > BODY_FONT_SIZE
    }
}

pub(super) fn extract_lines(
    html: &str,
    is_dark: bool,
) -> Result<Vec<NativeTextLine>, MarkdownError> {
    let body = body_content(html)?;
    let (body_no_code, code_blocks) = parser::extract_code_blocks(&body, is_dark)?;
    let with_heading_marks = parser::mark_headings(&body_no_code)?;
    let with_image_alt = parser::replace_image_alt(&with_heading_marks)?;
    let without_scripts = parser::remove_tag_blocks(&with_image_alt, "script")?;
    let without_style = parser::remove_tag_blocks(&without_scripts, "style")?;
    let with_breaks = parser::block_tags_to_breaks(&without_style)?;
    let without_tags = parser::strip_tags(&with_breaks);
    let decoded = parser::decode_entities(&without_tags);
    /* WHY: decode_entities() re-introduces < and > from HTML entities in code blocks.
    A second strip removes these decoded tag patterns. */
    let clean = parser::strip_tags(&decoded);
    Ok(parser::parse_typed_lines(&clean, &code_blocks))
}

fn body_content(html: &str) -> Result<String, MarkdownError> {
    parser::body_content(html)
}

pub(super) fn is_dark_background(color: &str) -> bool {
    parse_hex_rgb(color)
        .map(|[r, g, b]| {
            /* perceptual luminance */
            RED_LUMINANCE_WEIGHT * (r as f32)
                + GREEN_LUMINANCE_WEIGHT * (g as f32)
                + BLUE_LUMINANCE_WEIGHT * (b as f32)
                < DARK_THRESHOLD
        })
        .unwrap_or(false)
}

fn parse_hex_rgb(color: &str) -> Option<ColorTriplet> {
    let hex = color.trim().strip_prefix('#')?;
    let n = u32::from_str_radix(hex, HEX_RADIX).ok()?;
    match hex.len() {
        HEX_LONG_COLOR_LENGTH => Some([
            (n >> RED_SHIFT_LONG) as u8,
            (n >> GREEN_SHIFT_LONG) as u8,
            n as u8,
        ]),
        HEX_SHORT_COLOR_LENGTH => {
            let r = ((n >> SHORT_RED_SHIFT) & SHORT_COLOR_MASK) as u8;
            let g = ((n >> SHORT_GREEN_SHIFT) & SHORT_COLOR_MASK) as u8;
            let b = (n & SHORT_COLOR_MASK) as u8;
            let expand = |nibble: u8| -> u8 { (nibble << SHORT_COLOR_BITS) | nibble };
            Some([expand(r), expand(g), expand(b)])
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "native_text_tests.rs"]
mod tests;
