use super::{
    AlignmentBaseline, BaselineShift, DominantBaseline, Font, FontOpticalSizing, FontStretch,
    FontStyle, LengthAdjust, TextDecoration, TextDecorationStyle, TextSpan,
};
use crate::svg_engine::usvg::{
    Color, Fill, FillRule, LineCap, LineJoin, Opacity, Paint, PaintOrder, Stroke, StrokeMiterlimit,
    StrokeWidth,
};

#[path = "svg_engine_text_writer_model_tests.rs"]
mod writer_model_tests;

const EXPECTED_STROKE_WIDTH: f32 = 2.5;
const EXPECTED_STROKE_RGB: [u8; 3] = [17, 34, 51];
const EXPECTED_STROKE_MITER_LIMIT: f32 = 3.0;
const EXPECTED_FONT_WEIGHT: u16 = 600;
const EXPECTED_FILL_RGB: [u8; 3] = [90, 120, 150];
const EXPECTED_FONT_SIZE: f32 = 16.0;
const EXPECTED_SPAN_START_BYTE: usize = 2;
const EXPECTED_SPAN_END_BYTE: usize = 7;
const EXPECTED_LETTER_SPACING: f32 = 1.5;
const EXPECTED_WORD_SPACING: f32 = 2.5;
const EXPECTED_TEXT_LENGTH: f32 = 12.0;

#[test]
fn font_stretch_conversions_match_each_named_variant_in_both_directions() {
    let pairs = [
        (fontdb::Stretch::UltraCondensed, FontStretch::UltraCondensed),
        (fontdb::Stretch::ExtraCondensed, FontStretch::ExtraCondensed),
        (fontdb::Stretch::Condensed, FontStretch::Condensed),
        (fontdb::Stretch::SemiCondensed, FontStretch::SemiCondensed),
        (fontdb::Stretch::Normal, FontStretch::Normal),
        (fontdb::Stretch::SemiExpanded, FontStretch::SemiExpanded),
        (fontdb::Stretch::Expanded, FontStretch::Expanded),
        (fontdb::Stretch::ExtraExpanded, FontStretch::ExtraExpanded),
        (fontdb::Stretch::UltraExpanded, FontStretch::UltraExpanded),
    ];
    for (database, model) in pairs {
        assert_eq!(FontStretch::from(database), model);
        assert_eq!(fontdb::Stretch::from(model), database);
    }
    assert_eq!(FontStretch::default(), FontStretch::Normal);
}

#[test]
fn font_style_conversions_and_text_model_defaults_are_explicit() {
    let pairs = [
        (fontdb::Style::Normal, FontStyle::Normal),
        (fontdb::Style::Italic, FontStyle::Italic),
        (fontdb::Style::Oblique, FontStyle::Oblique),
    ];
    for (database, model) in pairs {
        assert_eq!(FontStyle::from(database), model);
        assert_eq!(fontdb::Style::from(model), database);
    }
    assert_eq!(FontStyle::default(), FontStyle::Normal);
    assert_eq!(BaselineShift::default(), BaselineShift::Baseline);
    assert_eq!(LengthAdjust::default(), LengthAdjust::Spacing);
    assert_eq!(FontOpticalSizing::default(), FontOpticalSizing::Auto);
}

fn expected_fill(color: Color) -> Fill {
    Fill {
        paint: Paint::Color(color),
        opacity: Opacity::ONE,
        rule: FillRule::NonZero,
        context_element: None,
    }
}

fn expected_stroke() -> Result<Stroke, String> {
    let width = StrokeWidth::new(EXPECTED_STROKE_WIDTH)
        .ok_or_else(|| "positive stroke width rejected".to_owned())?;
    Ok(Stroke {
        paint: Paint::Color(expected_stroke_color()),
        dasharray: None,
        dashoffset: 0.0,
        miterlimit: StrokeMiterlimit::new(EXPECTED_STROKE_MITER_LIMIT),
        opacity: Opacity::ONE,
        width,
        linecap: LineCap::Round,
        linejoin: LineJoin::Bevel,
        context_element: None,
    })
}

fn expected_fill_color() -> Color {
    Color::new_rgb(
        EXPECTED_FILL_RGB[0],
        EXPECTED_FILL_RGB[1],
        EXPECTED_FILL_RGB[2],
    )
}

fn expected_stroke_color() -> Color {
    Color::new_rgb(
        EXPECTED_STROKE_RGB[0],
        EXPECTED_STROKE_RGB[1],
        EXPECTED_STROKE_RGB[2],
    )
}

fn expected_font() -> Font {
    Font {
        families: vec![super::FontFamily::Serif],
        style: FontStyle::Italic,
        stretch: FontStretch::Expanded,
        weight: EXPECTED_FONT_WEIGHT,
        variations: Vec::new(),
    }
}

fn expected_decoration(fill_color: Color, stroke: Stroke) -> TextDecoration {
    TextDecoration {
        underline: Some(TextDecorationStyle {
            fill: Some(expected_fill(fill_color)),
            stroke: Some(stroke),
        }),
        overline: None,
        line_through: None,
    }
}

fn span_fixture() -> Result<TextSpan, String> {
    let fill_color = expected_fill_color();
    let stroke = expected_stroke()?;
    let decoration_stroke = stroke.clone();
    let font_size = crate::svg_engine::usvg::NonZeroPositiveF32::new(EXPECTED_FONT_SIZE)
        .ok_or_else(|| "positive font size rejected".to_owned())?;
    Ok(TextSpan {
        start: EXPECTED_SPAN_START_BYTE,
        end: EXPECTED_SPAN_END_BYTE,
        fill: Some(expected_fill(fill_color)),
        stroke: Some(stroke),
        paint_order: PaintOrder::StrokeAndFill,
        font: expected_font(),
        font_size,
        small_caps: true,
        apply_kerning: false,
        font_optical_sizing: FontOpticalSizing::None,
        decoration: expected_decoration(fill_color, decoration_stroke),
        dominant_baseline: DominantBaseline::Hanging,
        alignment_baseline: AlignmentBaseline::Middle,
        baseline_shift: vec![BaselineShift::Subscript, BaselineShift::Number(2.0)],
        visible: false,
        letter_spacing: EXPECTED_LETTER_SPACING,
        word_spacing: EXPECTED_WORD_SPACING,
        text_length: Some(EXPECTED_TEXT_LENGTH),
        length_adjust: LengthAdjust::SpacingAndGlyphs,
    })
}

fn assert_color(paint: &Paint, expected: Color) {
    assert!(matches!(paint, Paint::Color(actual) if *actual == expected));
}

fn assert_paint_getters(span: &TextSpan) -> Result<(), String> {
    let fill = span.fill().ok_or_else(|| "span fill missing".to_owned())?;
    assert_color(fill.paint(), expected_fill_color());
    let stroke = span
        .stroke()
        .ok_or_else(|| "span stroke missing".to_owned())?;
    assert_color(stroke.paint(), expected_stroke_color());
    assert_eq!(stroke.width().get(), EXPECTED_STROKE_WIDTH);
    assert_eq!(stroke.linecap(), LineCap::Round);
    assert_eq!(stroke.linejoin(), LineJoin::Bevel);
    Ok(())
}

fn assert_decoration_getters(span: &TextSpan) -> Result<(), String> {
    let decoration = span
        .decoration()
        .underline()
        .ok_or_else(|| "underline missing".to_owned())?;
    assert_color(
        decoration
            .fill()
            .ok_or_else(|| "underline fill missing".to_owned())?
            .paint(),
        expected_fill_color(),
    );
    let stroke = decoration
        .stroke()
        .ok_or_else(|| "underline stroke missing".to_owned())?;
    assert_color(stroke.paint(), expected_stroke_color());
    assert!(span.decoration().overline().is_none());
    assert!(span.decoration().line_through().is_none());
    Ok(())
}

fn assert_font_getters(span: &TextSpan) {
    assert_eq!(span.font().families(), &[super::FontFamily::Serif]);
    assert_eq!(span.font().style(), FontStyle::Italic);
    assert_eq!(span.font().stretch(), FontStretch::Expanded);
    assert_eq!(span.font().weight(), EXPECTED_FONT_WEIGHT);
    assert_eq!(span.font_size().get(), EXPECTED_FONT_SIZE);
    assert!(span.small_caps());
    assert!(!span.apply_kerning());
    assert_eq!(span.font_optical_sizing(), FontOpticalSizing::None);
}

fn assert_span_range_and_order(span: &TextSpan) {
    assert_eq!(
        (span.start(), span.end()),
        (EXPECTED_SPAN_START_BYTE, EXPECTED_SPAN_END_BYTE)
    );
    assert_eq!(span.paint_order(), PaintOrder::StrokeAndFill);
}

fn assert_span_baselines(span: &TextSpan) {
    assert_eq!(span.dominant_baseline(), DominantBaseline::Hanging);
    assert_eq!(span.alignment_baseline(), AlignmentBaseline::Middle);
    assert_eq!(
        span.baseline_shift(),
        &[BaselineShift::Subscript, BaselineShift::Number(2.0)]
    );
}

fn assert_span_spacing(span: &TextSpan) {
    assert!(!span.is_visible());
    assert_eq!(span.letter_spacing(), EXPECTED_LETTER_SPACING);
    assert_eq!(span.word_spacing(), EXPECTED_WORD_SPACING);
    assert_eq!(span.text_length(), Some(EXPECTED_TEXT_LENGTH));
    assert_eq!(span.length_adjust(), LengthAdjust::SpacingAndGlyphs);
}

#[test]
fn text_span_and_decoration_getters_preserve_authored_model_values() -> Result<(), String> {
    let span = span_fixture()?;
    assert_paint_getters(&span)?;
    assert_decoration_getters(&span)?;
    assert_font_getters(&span);
    assert_span_range_and_order(&span);
    assert_span_baselines(&span);
    assert_span_spacing(&span);
    Ok(())
}
