// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use kurbo::{ParamCurve, ParamCurveArclen};
use svgtypes::{FontFamily, Length, LengthUnit, parse_font_families};

use super::svgtree::{AId, EId, FromValue, SvgNode};
use super::{OptionLog, converter, style};
use crate::svg_engine::usvg::*;

impl<'a, 'input: 'a> FromValue<'a, 'input> for TextAnchor {
    fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
        match value {
            "start" => Some(TextAnchor::Start),
            "middle" => Some(TextAnchor::Middle),
            "end" => Some(TextAnchor::End),
            _ => None,
        }
    }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for AlignmentBaseline {
    fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
        match value {
            "auto" => Some(AlignmentBaseline::Auto),
            "baseline" => Some(AlignmentBaseline::Baseline),
            "before-edge" => Some(AlignmentBaseline::BeforeEdge),
            "text-before-edge" => Some(AlignmentBaseline::TextBeforeEdge),
            "middle" => Some(AlignmentBaseline::Middle),
            "central" => Some(AlignmentBaseline::Central),
            "after-edge" => Some(AlignmentBaseline::AfterEdge),
            "text-after-edge" => Some(AlignmentBaseline::TextAfterEdge),
            "ideographic" => Some(AlignmentBaseline::Ideographic),
            "alphabetic" => Some(AlignmentBaseline::Alphabetic),
            "hanging" => Some(AlignmentBaseline::Hanging),
            "mathematical" => Some(AlignmentBaseline::Mathematical),
            _ => None,
        }
    }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for DominantBaseline {
    fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
        match value {
            "auto" => Some(DominantBaseline::Auto),
            "use-script" => Some(DominantBaseline::UseScript),
            "no-change" => Some(DominantBaseline::NoChange),
            "reset-size" => Some(DominantBaseline::ResetSize),
            "ideographic" => Some(DominantBaseline::Ideographic),
            "alphabetic" => Some(DominantBaseline::Alphabetic),
            "hanging" => Some(DominantBaseline::Hanging),
            "mathematical" => Some(DominantBaseline::Mathematical),
            "central" => Some(DominantBaseline::Central),
            "middle" => Some(DominantBaseline::Middle),
            "text-after-edge" => Some(DominantBaseline::TextAfterEdge),
            "text-before-edge" => Some(DominantBaseline::TextBeforeEdge),
            _ => None,
        }
    }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for LengthAdjust {
    fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
        match value {
            "spacing" => Some(LengthAdjust::Spacing),
            "spacingAndGlyphs" => Some(LengthAdjust::SpacingAndGlyphs),
            _ => None,
        }
    }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for FontStyle {
    fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
        match value {
            "normal" => Some(FontStyle::Normal),
            "italic" => Some(FontStyle::Italic),
            "oblique" => Some(FontStyle::Oblique),
            _ => None,
        }
    }
}

/// A text character position.
///
/// _Character_ is a Unicode codepoint.
#[derive(Clone, Copy, Debug)]
struct CharacterPosition {
    /// An absolute X axis position.
    x: Option<f32>,
    /// An absolute Y axis position.
    y: Option<f32>,
    /// A relative X axis offset.
    dx: Option<f32>,
    /// A relative Y axis offset.
    dy: Option<f32>,
}

pub(crate) fn convert(
    text_node: SvgNode,
    state: &converter::State,
    cache: &mut converter::Cache,
    parent: &mut Group,
) {
    let pos_list = resolve_positions_list(text_node, state);
    let rotate_list = resolve_rotate_list(text_node);
    let writing_mode = convert_writing_mode(text_node);

    let chunks = collect_text_chunks(text_node, &pos_list, state, cache);

    let rendering_mode: TextRendering = text_node
        .find_attribute(AId::TextRendering)
        .unwrap_or(state.opt.text_rendering);

    let id = text_element_id(text_node, state);

    let mut text = new_text(
        id,
        rendering_mode,
        &pos_list,
        rotate_list,
        writing_mode,
        chunks,
        parent.abs_transform,
    );

    if text::convert(&mut text, &state.opt.font_resolver, cache).is_none() {
        return;
    }

    parent.children.push(Node::Text(Box::new(text)));
}

fn text_element_id(node: SvgNode, state: &converter::State) -> String {
    // markerから生成したnodeのIDは重複するため付けない。
    if state.parent_markers.is_empty() {
        node.element_id().to_string()
    } else {
        String::new()
    }
}

fn new_text(
    id: String,
    rendering_mode: TextRendering,
    positions: &[CharacterPosition],
    rotate: Vec<f32>,
    writing_mode: WritingMode,
    chunks: Vec<TextChunk>,
    abs_transform: Transform,
) -> Text {
    let dummy = Rect::from_xywh(0.0, 0.0, 0.0, 0.0).unwrap();
    Text {
        id,
        rendering_mode,
        dx: positions.iter().map(|v| v.dx.unwrap_or(0.0)).collect(),
        dy: positions.iter().map(|v| v.dy.unwrap_or(0.0)).collect(),
        rotate,
        writing_mode,
        chunks,
        abs_transform,
        // All fields below will be reset by `text_to_paths`.
        bounding_box: dummy,
        abs_bounding_box: dummy,
        stroke_bounding_box: dummy,
        abs_stroke_bounding_box: dummy,
        flattened: Box::new(Group::empty()),
        layouted: vec![],
    }
}

struct IterState {
    chars_count: usize,
    chunk_bytes_count: usize,
    split_chunk: bool,
    text_flow: TextFlow,
    chunks: Vec<TextChunk>,
}

fn collect_text_chunks(
    text_node: SvgNode,
    pos_list: &[CharacterPosition],
    state: &converter::State,
    cache: &mut converter::Cache,
) -> Vec<TextChunk> {
    let mut iter_state = IterState {
        chars_count: 0,
        chunk_bytes_count: 0,
        split_chunk: false,
        text_flow: TextFlow::Linear,
        chunks: Vec::new(),
    };

    collect_text_chunks_impl(text_node, pos_list, state, cache, &mut iter_state);

    iter_state.chunks
}

fn collect_text_chunks_impl(
    parent: SvgNode,
    pos_list: &[CharacterPosition],
    state: &converter::State,
    cache: &mut converter::Cache,
    iter_state: &mut IterState,
) {
    for child in parent.children() {
        if child.is_element() {
            collect_element_chunks(child, pos_list, state, cache, iter_state);
            continue;
        }

        collect_text_child(parent, child, pos_list, state, cache, iter_state);
    }
}

fn collect_text_child(
    parent: SvgNode,
    child: SvgNode,
    pos_list: &[CharacterPosition],
    state: &converter::State,
    cache: &mut converter::Cache,
    iter_state: &mut IterState,
) {
    if !parent.is_visible_element(state.opt) {
        iter_state.chars_count += child.text().chars().count();
        return;
    }

    let anchor = parent.find_attribute(AId::TextAnchor).unwrap_or_default();

    let Some(span) = resolve_text_span(parent, state, cache) else {
        iter_state.chars_count += child.text().chars().count();
        return;
    };

    append_text_characters(child, &span, anchor, pos_list, iter_state);
}

struct TextSpanProperties {
    font_size: NonZeroPositiveF32,
    font: Font,
    paint_order: PaintOrder,
    dominant_baseline: DominantBaseline,
    apply_kerning: bool,
    font_optical_sizing: FontOpticalSizing,
    text_length: Option<f32>,
    visibility: Visibility,
}

fn resolve_text_span(
    parent: SvgNode,
    state: &converter::State,
    cache: &mut converter::Cache,
) -> Option<TextSpan> {
    let properties = resolve_text_span_properties(parent, state)?;
    Some(TextSpan {
        start: 0,
        end: 0,
        fill: style::resolve_fill(parent, true, state, cache),
        stroke: style::resolve_stroke(parent, true, state, cache),
        paint_order: properties.paint_order,
        font: properties.font,
        font_size: properties.font_size,
        small_caps: parent.find_attribute::<&str>(AId::FontVariant) == Some("small-caps"),
        apply_kerning: properties.apply_kerning,
        font_optical_sizing: properties.font_optical_sizing,
        decoration: resolve_decoration(parent, state, cache),
        visible: properties.visibility == Visibility::Visible,
        dominant_baseline: properties.dominant_baseline,
        alignment_baseline: parent
            .find_attribute(AId::AlignmentBaseline)
            .unwrap_or_default(),
        baseline_shift: convert_baseline_shift(parent, state),
        letter_spacing: parent.resolve_length(AId::LetterSpacing, state, 0.0),
        word_spacing: parent.resolve_length(AId::WordSpacing, state, 0.0),
        text_length: properties.text_length,
        length_adjust: parent.find_attribute(AId::LengthAdjust).unwrap_or_default(),
    })
}

fn resolve_text_span_properties(
    parent: SvgNode,
    state: &converter::State,
) -> Option<TextSpanProperties> {
    // TODO: what to do when <= 0? UB?
    let font_size = super::units::resolve_font_size(parent, state);
    let font_size = NonZeroPositiveF32::new(font_size)?;

    let font = convert_font(parent, state);

    let raw_paint_order: svgtypes::PaintOrder =
        parent.find_attribute(AId::PaintOrder).unwrap_or_default();
    let paint_order = super::converter::svg_paint_order_to_usvg(raw_paint_order);

    let dominant_baseline = resolve_dominant_baseline(parent);
    let apply_kerning = resolve_kerning(parent, state);

    let font_optical_sizing = resolve_optical_sizing(parent);

    let text_length = resolve_text_length(parent, state);

    let visibility: Visibility = parent.find_attribute(AId::Visibility).unwrap_or_default();

    Some(TextSpanProperties {
        font_size,
        font,
        paint_order,
        dominant_baseline,
        apply_kerning,
        font_optical_sizing,
        text_length,
        visibility,
    })
}

fn resolve_optical_sizing(parent: SvgNode) -> FontOpticalSizing {
    match parent.find_attribute::<&str>(AId::FontOpticalSizing) {
        Some("none") => FontOpticalSizing::None,
        _ => FontOpticalSizing::Auto,
    }
}

fn resolve_dominant_baseline(parent: SvgNode) -> DominantBaseline {
    let mut baseline = parent
        .find_attribute(AId::DominantBaseline)
        .unwrap_or_default();
    if baseline == DominantBaseline::NoChange {
        baseline = parent
            .parent_element()
            .unwrap()
            .find_attribute(AId::DominantBaseline)
            .unwrap_or_default();
    }
    baseline
}

fn resolve_kerning(parent: SvgNode, state: &converter::State) -> bool {
    if parent.resolve_length(AId::Kerning, state, -1.0) == 0.0 {
        false
    } else {
        parent.find_attribute::<&str>(AId::FontKerning) != Some("none")
    }
}

fn resolve_text_length(parent: SvgNode, state: &converter::State) -> Option<f32> {
    let mut length = parent.try_convert_length(AId::TextLength, Units::UserSpaceOnUse, state);
    if let Some(n) = length {
        if n < 0.0 {
            length = None;
        }
    }
    length
}

fn append_text_characters(
    child: SvgNode,
    span: &TextSpan,
    anchor: TextAnchor,
    pos_list: &[CharacterPosition],
    iter_state: &mut IterState,
) {
    let mut is_new_span = true;
    for c in child.text().chars() {
        let char_len = c.len_utf8();

        let is_new_chunk = starts_text_chunk(pos_list, iter_state);

        iter_state.split_chunk = false;

        if is_new_chunk {
            start_text_chunk(c, span, anchor, pos_list, iter_state);
        } else if is_new_span {
            append_text_span(c, span, iter_state);
        } else {
            extend_text_span(c, iter_state);
        }

        is_new_span = false;
        iter_state.chars_count += 1;
        iter_state.chunk_bytes_count += char_len;
    }
}

fn starts_text_chunk(pos_list: &[CharacterPosition], iter_state: &IterState) -> bool {
    // 絶対座標、textPathの境界、または最初の文字でchunkを分ける。
    pos_list[iter_state.chars_count].x.is_some()
        || pos_list[iter_state.chars_count].y.is_some()
        || iter_state.split_chunk
        || iter_state.chunks.is_empty()
}

fn start_text_chunk(
    c: char,
    span: &TextSpan,
    anchor: TextAnchor,
    pos_list: &[CharacterPosition],
    iter_state: &mut IterState,
) {
    iter_state.chunk_bytes_count = 0;
    let mut span = span.clone();
    span.start = 0;
    span.end = c.len_utf8();
    iter_state.chunks.push(TextChunk {
        x: pos_list[iter_state.chars_count].x,
        y: pos_list[iter_state.chars_count].y,
        anchor,
        spans: vec![span],
        text_flow: iter_state.text_flow.clone(),
        text: c.to_string(),
    });
}

fn append_text_span(c: char, span: &TextSpan, iter_state: &mut IterState) {
    let mut span = span.clone();
    span.start = iter_state.chunk_bytes_count;
    span.end = iter_state.chunk_bytes_count + c.len_utf8();
    if let Some(chunk) = iter_state.chunks.last_mut() {
        chunk.text.push(c);
        chunk.spans.push(span);
    }
}

fn extend_text_span(c: char, iter_state: &mut IterState) {
    if let Some(chunk) = iter_state.chunks.last_mut() {
        chunk.text.push(c);
        if let Some(span) = chunk.spans.last_mut() {
            debug_assert_ne!(span.end, 0);
            span.end += c.len_utf8();
        }
    }
}

fn collect_element_chunks(
    child: SvgNode,
    pos_list: &[CharacterPosition],
    state: &converter::State,
    cache: &mut converter::Cache,
    iter_state: &mut IterState,
) {
    if child.tag_name() == Some(EId::TextPath) {
        match resolve_text_flow(child, state) {
            Some(v) => iter_state.text_flow = v,
            None => {
                // 無効なパスを飛ばす場合も文字位置の数には含める。
                iter_state.chars_count += count_chars(child);
                return;
            }
        }
        iter_state.split_chunk = true;
    }
    collect_text_chunks_impl(child, pos_list, state, cache, iter_state);
    iter_state.text_flow = TextFlow::Linear;
    // textPath直後の文字も新しいchunkにする。
    if child.tag_name() == Some(EId::TextPath) {
        iter_state.split_chunk = true;
    }
}

fn resolve_text_flow(node: SvgNode, state: &converter::State) -> Option<TextFlow> {
    let linked_node = node.attribute::<SvgNode>(AId::Href)?;
    let path = super::shapes::convert(linked_node, state)?;

    // The reference path's transform needs to be applied
    let transform = linked_node.resolve_transform(AId::Transform, state);
    let path = if !transform.is_identity() {
        let mut path_copy = path.as_ref().clone();
        path_copy = path_copy.transform(transform)?;
        Arc::new(path_copy)
    } else {
        path
    };

    let start_offset: Length = node.attribute(AId::StartOffset).unwrap_or_default();
    let start_offset = if start_offset.unit == LengthUnit::Percent {
        // 'If a percentage is given, then the `startOffset` represents
        // a percentage distance along the entire path.'
        let path_len = path_length(&path);
        (path_len * (start_offset.number / 100.0)) as f32
    } else {
        node.resolve_length(AId::StartOffset, state, 0.0)
    };

    let id = NonEmptyString::new(linked_node.element_id().to_string())?;
    Some(TextFlow::Path(Arc::new(TextPath {
        id,
        start_offset,
        path,
    })))
}

pub(super) fn convert_font(node: SvgNode, state: &converter::State) -> Font {
    let style: FontStyle = node.find_attribute(AId::FontStyle).unwrap_or_default();
    let stretch = conv_font_stretch(node);
    let weight = resolve_font_weight(node);
    let mut variations = parse_font_variation_settings(node);
    add_standard_variations(&mut variations, style, stretch, weight);
    let families = resolve_font_families(node, state);

    Font {
        families,
        style,
        stretch,
        weight,
        variations,
    }
}

fn resolve_font_families(node: SvgNode, state: &converter::State) -> Vec<FontFamily> {
    let font_families = if let Some(n) = node.ancestors().find(|n| n.has_attribute(AId::FontFamily))
    {
        n.attribute(AId::FontFamily).unwrap_or("")
    } else {
        ""
    };

    let mut families = parse_font_families(font_families)
        .ok()
        .log_none(|| {
            log::warn!(
                "Failed to parse {} value: '{}'. Falling back to {}.",
                AId::FontFamily,
                font_families,
                state.opt.font_family
            );
        })
        .unwrap_or_default();

    if families.is_empty() {
        families.push(FontFamily::Named(state.opt.font_family.clone()));
    }

    families
}

fn add_standard_variations(
    variations: &mut Vec<FontVariation>,
    style: FontStyle,
    stretch: FontStretch,
    weight: u16,
) {
    // Auto-map standard font properties to variation axes if not explicitly set.
    // This allows variable fonts to work with regular font-weight/font-stretch properties.
    let has_wght = variations.iter().any(|v| &v.tag == b"wght");
    let has_wdth = variations.iter().any(|v| &v.tag == b"wdth");
    let has_ital = variations.iter().any(|v| &v.tag == b"ital");
    let has_slnt = variations.iter().any(|v| &v.tag == b"slnt");

    // Map font-weight to wght axis (if not already set)
    if !has_wght {
        variations.push(FontVariation::new(*b"wght", weight as f32));
    }

    // Map font-stretch to wdth axis (if not already set)
    // CSS font-stretch percentages: ultra-condensed=50%, condensed=75%, normal=100%, expanded=125%, ultra-expanded=200%
    if !has_wdth {
        let wdth = font_stretch_percentage(stretch);
        if wdth != 100.0 {
            variations.push(FontVariation::new(*b"wdth", wdth));
        }
    }

    add_style_variations(variations, style, has_ital, has_slnt);
}

fn add_style_variations(
    variations: &mut Vec<FontVariation>,
    style: FontStyle,
    has_ital: bool,
    has_slnt: bool,
) {
    // Map font-style: italic to ital axis (if not already set)
    if !has_ital && style == FontStyle::Italic {
        variations.push(FontVariation::new(*b"ital", 1.0));
    }

    // Map font-style: oblique to slnt axis (if not already set)
    // Default oblique angle is typically 12-14 degrees
    if !has_slnt && style == FontStyle::Oblique {
        variations.push(FontVariation::new(*b"slnt", -12.0));
    }
}

fn font_stretch_percentage(stretch: FontStretch) -> f32 {
    match stretch {
        FontStretch::UltraCondensed => 50.0,
        FontStretch::ExtraCondensed => 62.5,
        FontStretch::Condensed => 75.0,
        FontStretch::SemiCondensed => 87.5,
        FontStretch::Normal => 100.0,
        FontStretch::SemiExpanded => 112.5,
        FontStretch::Expanded => 125.0,
        FontStretch::ExtraExpanded => 150.0,
        FontStretch::UltraExpanded => 200.0,
    }
}

/// Parses the `font-variation-settings` CSS property.
///
/// Syntax: `normal | [ <string> <number> ]#`
/// Example: `"wght" 700, "wdth" 50`
fn parse_font_variation_settings(node: SvgNode) -> Vec<FontVariation> {
    let Some(value) = font_variation_settings_value(node) else {
        return Vec::new();
    };

    // "normal" means no variations
    if value.eq_ignore_ascii_case("normal") || value.is_empty() {
        return Vec::new();
    }

    let mut variations = Vec::new();

    // Parse comma-separated list of "tag" value pairs
    for part in value.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(variation) = parse_variation_part(part) {
            variations.push(variation);
        }
    }

    variations
}

fn font_variation_settings_value<'a, 'input: 'a>(node: SvgNode<'a, 'input>) -> Option<&'a str> {
    let n = node
        .ancestors()
        .find(|n| n.has_attribute(AId::FontVariationSettings))?;
    let value = n.attribute(AId::FontVariationSettings).unwrap_or("");
    log::debug!("Found font-variation-settings: '{}'", value);
    Some(value)
}

fn parse_variation_part(part: &str) -> Option<FontVariation> {
    let mut chars = part.chars().peekable();
    let tag_str = parse_variation_tag(&mut chars)?;
    while chars.peek().map_or(false, |c| c.is_whitespace()) {
        chars.next();
    }
    let value_str: String = chars.collect();
    let value_str = value_str.trim();
    let value = match value_str.parse::<f32>() {
        Ok(v) => v,
        Err(_) => {
            log::warn!("Invalid font-variation-settings value: '{}'", value_str);
            return None;
        }
    };
    let tag_bytes = tag_str.as_bytes();
    let tag = [tag_bytes[0], tag_bytes[1], tag_bytes[2], tag_bytes[3]];
    Some(FontVariation::new(tag, value))
}

fn parse_variation_tag(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<String> {
    let quote = match chars.next() {
        Some('"') => '"',
        Some('\'') => '\'',
        _ => return None,
    };
    let mut tag = String::new();
    for c in chars.by_ref() {
        if c == quote {
            break;
        }
        tag.push(c);
    }
    if tag.len() != 4 {
        log::warn!(
            "Invalid font-variation-settings tag: '{}' (must be 4 characters)",
            tag
        );
        return None;
    }
    Some(tag)
}

// TODO: properly resolve narrower/wider
fn conv_font_stretch(node: SvgNode) -> FontStretch {
    if let Some(n) = node.ancestors().find(|n| n.has_attribute(AId::FontStretch)) {
        match n.attribute(AId::FontStretch).unwrap_or("") {
            "narrower" | "condensed" => FontStretch::Condensed,
            "ultra-condensed" => FontStretch::UltraCondensed,
            "extra-condensed" => FontStretch::ExtraCondensed,
            "semi-condensed" => FontStretch::SemiCondensed,
            "semi-expanded" => FontStretch::SemiExpanded,
            "wider" | "expanded" => FontStretch::Expanded,
            "extra-expanded" => FontStretch::ExtraExpanded,
            "ultra-expanded" => FontStretch::UltraExpanded,
            _ => FontStretch::Normal,
        }
    } else {
        FontStretch::Normal
    }
}

fn resolve_font_weight(node: SvgNode) -> u16 {
    let nodes: Vec<_> = node.ancestors().collect();
    let mut weight = 400;
    for n in nodes.iter().rev().skip(1) {
        // skip Root
        weight = match n.attribute(AId::FontWeight).unwrap_or("") {
            "normal" => 400,
            "bold" => 700,
            "100" => 100,
            "200" => 200,
            "300" => 300,
            "400" => 400,
            "500" => 500,
            "600" => 600,
            "700" => 700,
            "800" => 800,
            "900" => 900,
            "bolder" => relative_font_weight(weight, true),
            "lighter" => relative_font_weight(weight, false),
            _ => weight,
        };
    }

    weight as u16
}

fn relative_font_weight(weight: usize, bolder: bool) -> usize {
    // 既定の太さではChromeとInkscapeの挙動に合わせる。
    let value = if bolder {
        weight + if weight == 400 { 300 } else { 100 }
    } else {
        weight - if weight == 400 { 200 } else { 100 }
    };
    value.clamp(100, 900)
}

/// Resolves text's character positions.
///
/// This includes: x, y, dx, dy.
///
/// # The character
///
/// The first problem with this task is that the *character* itself
/// is basically undefined in the SVG spec. Sometimes it's an *XML character*,
/// sometimes a *glyph*, and sometimes just a *character*.
///
/// There is an ongoing [discussion](https://github.com/w3c/svgwg/issues/537)
/// on the SVG working group that addresses this by stating that a character
/// is a Unicode code point. But it's not final.
///
/// Also, according to the SVG 2 spec, *character* is *a Unicode code point*.
///
/// Anyway, we treat a character as a Unicode code point.
///
/// # Algorithm
///
/// To resolve positions, we have to iterate over descendant nodes and
/// if the current node is a `tspan` and has x/y/dx/dy attribute,
/// than the positions from this attribute should be assigned to the characters
/// of this `tspan` and it's descendants.
///
/// Positions list can have more values than characters in the `tspan`,
/// so we have to clamp it, because values should not overlap, e.g.:
///
/// (we ignore whitespaces for example purposes,
/// so the `text` content is `Text` and not `T ex t`)
///
/// ```text
/// <text>
///   a
///   <tspan x="10 20 30">
///     bc
///   </tspan>
///   d
/// </text>
/// ```
///
/// In this example, the `d` position should not be set to `30`.
/// And the result should be: `[None, 10, 20, None]`
///
/// Another example:
///
/// ```text
/// <text>
///   <tspan x="100 110 120 130">
///     a
///     <tspan x="50">
///       bc
///     </tspan>
///   </tspan>
///   d
/// </text>
/// ```
///
/// The result should be: `[100, 50, 120, None]`
fn resolve_positions_list(text_node: SvgNode, state: &converter::State) -> Vec<CharacterPosition> {
    // Allocate a list that has all characters positions set to `None`.
    let total_chars = count_chars(text_node);
    let mut list = vec![
        CharacterPosition {
            x: None,
            y: None,
            dx: None,
            dy: None,
        };
        total_chars
    ];

    let mut offset = 0;
    for child in text_node.descendants() {
        if child.is_element() {
            // We must ignore text positions on `textPath`.
            if !matches!(child.tag_name(), Some(EId::Text) | Some(EId::Tspan)) {
                continue;
            }

            resolve_element_positions(child, state, &mut list, offset);
        } else if child.is_text() {
            // Advance the offset.
            offset += child.text().chars().count();
        }
    }

    list
}

fn resolve_element_positions(
    child: SvgNode,
    state: &converter::State,
    list: &mut [CharacterPosition],
    offset: usize,
) {
    let child_chars = count_chars(child);
    macro_rules! push_list {
        ($aid:expr, $field:ident) => {
            if let Some(num_list) = super::units::convert_list(child, $aid, state) {
                let len = std::cmp::min(num_list.len(), child_chars);
                for i in 0..len {
                    list[offset + i].$field = Some(num_list[i]);
                }
            }
        };
    }
    push_list!(AId::X, x);
    push_list!(AId::Y, y);
    push_list!(AId::Dx, dx);
    push_list!(AId::Dy, dy);
}

/// Resolves characters rotation.
///
/// The algorithm is well explained
/// [in the SVG spec](https://www.w3.org/TR/SVG11/text.html#TSpanElement) (scroll down a bit).
///
/// ![](https://www.w3.org/TR/SVG11/images/text/tspan05-diagram.png)
///
/// Note: this algorithm differs from the position resolving one.
fn resolve_rotate_list(text_node: SvgNode) -> Vec<f32> {
    // Allocate a list that has all characters angles set to `0.0`.
    let mut list = vec![0.0; count_chars(text_node)];
    let mut last = 0.0;
    let mut offset = 0;
    for child in text_node.descendants() {
        if child.is_element() {
            if let Some(rotate) = child.attribute::<Vec<f32>>(AId::Rotate) {
                for i in 0..count_chars(child) {
                    if let Some(a) = rotate.get(i).cloned() {
                        list[offset + i] = a;
                        last = a;
                    } else {
                        // If the rotate list doesn't specify the rotation for
                        // this character - use the last one.
                        list[offset + i] = last;
                    }
                }
            }
        } else if child.is_text() {
            // Advance the offset.
            offset += child.text().chars().count();
        }
    }

    list
}

/// Resolves node's `text-decoration` property.
fn resolve_decoration(
    tspan: SvgNode,
    state: &converter::State,
    cache: &mut converter::Cache,
) -> TextDecoration {
    // The algorithm is as follows: First, we check whether the given text decoration appears in ANY
    // ancestor, i.e. it can also appear in ancestors outside of the <text> element. If the text
    // decoration is declared somewhere, it means that this tspan will have it. However, we still
    // need to find the corresponding fill/stroke for it. To do this, we iterate through all
    // ancestors (i.e. tspans) until we find the text decoration declared. If not, we will
    // stop at latest at the text node, and use its fill/stroke.
    TextDecoration {
        underline: decoration_style(tspan, "underline", state, cache),
        overline: decoration_style(tspan, "overline", state, cache),
        line_through: decoration_style(tspan, "line-through", state, cache),
    }
}

fn find_decoration(node: SvgNode, value: &str) -> bool {
    if let Some(str_value) = node.attribute::<&str>(AId::TextDecoration) {
        str_value.split(' ').any(|v| v == value)
    } else {
        false
    }
}

fn decoration_style(
    tspan: SvgNode,
    text_decoration: &str,
    state: &converter::State,
    cache: &mut converter::Cache,
) -> Option<TextDecorationStyle> {
    if !tspan
        .ancestors()
        .any(|n| find_decoration(n, text_decoration))
    {
        return None;
    }
    let mut fill_node = None;
    let mut stroke_node = None;
    for node in tspan.ancestors() {
        if find_decoration(node, text_decoration) || node.tag_name() == Some(EId::Text) {
            fill_node = fill_node.map_or(Some(node), Some);
            stroke_node = stroke_node.map_or(Some(node), Some);
            break;
        }
    }
    Some(TextDecorationStyle {
        fill: fill_node.and_then(|node| style::resolve_fill(node, true, state, cache)),
        stroke: stroke_node.and_then(|node| style::resolve_stroke(node, true, state, cache)),
    })
}

fn convert_baseline_shift(node: SvgNode, state: &converter::State) -> Vec<BaselineShift> {
    let mut shift = Vec::new();
    let nodes: Vec<_> = node
        .ancestors()
        .take_while(|n| n.tag_name() != Some(EId::Text))
        .collect();
    for n in nodes {
        if let Some(value) = resolve_baseline_shift(n, state) {
            shift.push(value);
        }
    }

    if shift
        .iter()
        .all(|base| matches!(base, BaselineShift::Baseline))
    {
        shift.clear();
    }

    shift
}

fn resolve_baseline_shift(node: SvgNode, state: &converter::State) -> Option<BaselineShift> {
    if let Some(len) = node.try_attribute::<Length>(AId::BaselineShift) {
        let value = if len.unit == LengthUnit::Percent {
            super::units::resolve_font_size(node, state) * (len.number as f32 / 100.0)
        } else {
            super::units::convert_length(
                len,
                node,
                AId::BaselineShift,
                Units::ObjectBoundingBox,
                state,
            )
        };
        Some(BaselineShift::Number(value))
    } else {
        node.attribute(AId::BaselineShift).map(|s| match s {
            "sub" => BaselineShift::Subscript,
            "super" => BaselineShift::Superscript,
            _ => BaselineShift::Baseline,
        })
    }
}

fn count_chars(node: SvgNode) -> usize {
    node.descendants()
        .filter(|n| n.is_text())
        .fold(0, |w, n| w + n.text().chars().count())
}

/// Converts the writing mode.
///
/// [SVG 2] references [CSS Writing Modes Level 3] for the definition of the
/// 'writing-mode' property, there are only two writing modes:
/// horizontal left-to-right and vertical right-to-left.
///
/// That specification introduces new values for the property. The SVG 1.1
/// values are obsolete but must still be supported by converting the specified
/// values to computed values as follows:
///
/// - `lr`, `lr-tb`, `rl`, `rl-tb` => `horizontal-tb`
/// - `tb`, `tb-rl` => `vertical-rl`
///
/// The current `vertical-lr` behaves exactly the same as `vertical-rl`.
///
/// Also, looks like no one really supports the `rl` and `rl-tb`, except `Batik`.
/// And I'm not sure if its behaviour is correct.
///
/// So we will ignore it as well, mainly because I have no idea how exactly
/// it should affect the rendering.
///
/// [SVG 2]: https://www.w3.org/TR/SVG2/text.html#WritingModeProperty
/// [CSS Writing Modes Level 3]: https://www.w3.org/TR/css-writing-modes-3/#svg-writing-mode-css
fn convert_writing_mode(text_node: SvgNode) -> WritingMode {
    if let Some(n) = text_node
        .ancestors()
        .find(|n| n.has_attribute(AId::WritingMode))
    {
        match n.attribute(AId::WritingMode).unwrap_or("lr-tb") {
            "tb" | "tb-rl" | "vertical-rl" | "vertical-lr" => WritingMode::TopToBottom,
            _ => WritingMode::LeftToRight,
        }
    } else {
        WritingMode::LeftToRight
    }
}

fn path_length(path: &tiny_skia_path::Path) -> f64 {
    let first = path.points()[0];
    let mut previous = (first.x, first.y);
    let mut start = previous;

    let mut length = 0.0;
    for seg in path.segments() {
        if let Some(curve) = path_segment_curve(seg, &mut previous, &mut start) {
            length += curve.arclen(0.5);
            previous = (curve.p3.x as f32, curve.p3.y as f32);
        }
    }

    length
}

fn path_segment_curve(
    seg: tiny_skia_path::PathSegment,
    previous: &mut (f32, f32),
    start: &mut (f32, f32),
) -> Option<kurbo::CubicBez> {
    match seg {
        tiny_skia_path::PathSegment::MoveTo(p) => {
            *start = (p.x, p.y);
            *previous = *start;
            None
        }
        tiny_skia_path::PathSegment::LineTo(p) => Some(create_curve_from_line(previous.0, previous.1, p.x, p.y)),
        tiny_skia_path::PathSegment::QuadTo(p1, p) => Some(
            kurbo::QuadBez::new(
                kurbo::Point::new(previous.0 as f64, previous.1 as f64),
                kurbo::Point::new(p1.x as f64, p1.y as f64),
                kurbo::Point::new(p.x as f64, p.y as f64),
            )
            .raise(),
        ),
        tiny_skia_path::PathSegment::CubicTo(p1, p2, p) => Some(kurbo::CubicBez::new(
            kurbo::Point::new(previous.0 as f64, previous.1 as f64),
            kurbo::Point::new(p1.x as f64, p1.y as f64),
            kurbo::Point::new(p2.x as f64, p2.y as f64),
            kurbo::Point::new(p.x as f64, p.y as f64),
        )),
        tiny_skia_path::PathSegment::Close => Some(create_curve_from_line(previous.0, previous.1, start.0, start.1)),
    }
}

fn create_curve_from_line(px: f32, py: f32, x: f32, y: f32) -> kurbo::CubicBez {
    let line = kurbo::Line::new(
        kurbo::Point::new(px as f64, py as f64),
        kurbo::Point::new(x as f64, y as f64),
    );
    let p1 = line.eval(0.33);
    let p2 = line.eval(0.66);
    kurbo::CubicBez::new(line.p0, p1, p2, line.p1)
}
