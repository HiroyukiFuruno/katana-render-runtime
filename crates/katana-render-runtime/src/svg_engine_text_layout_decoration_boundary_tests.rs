use super::{FONT, TestResult, database_with_font, parse, text, usvg};
use rstest::rstest;
use tiny_skia::Transform;
use usvg::layout::{DatabaseExt, DecorationSpan, ResolvedFont, convert_decoration};

const VALID_DECORATION_WIDTH: f32 = 10.0;
const STRIKEOUT_FALLBACK_HEIGHT_RATIO: f32 = 0.45;
const UNDERLINE_FALLBACK_THICKNESS_DIVISOR: u16 = 12;
const SFNT_DIRECTORY_OFFSET: usize = 12;
const SFNT_RECORD_SIZE: usize = 16;
const SFNT_TAG_SIZE: usize = 4;

#[test]
fn standalone_combining_mark_keeps_ink_without_zero_width_underline() -> TestResult<()> {
    const ACCENT: &str = "\u{0301}";
    let decorated = parse("text-decoration='underline'", ACCENT)?;
    let reference = parse("", ACCENT)?;
    let layout = text(&decorated)?.layouted().first().ok_or("span missing")?;
    let [glyph] = layout.positioned_glyphs.as_slice() else {
        return Err("expected a standalone combining glyph".into());
    };
    assert_ne!(glyph.id.0, 0);
    assert!(layout.underline.is_none());
    let pixels = raster(&decorated)?;
    assert!(pixels.pixels().iter().any(|pixel| pixel.alpha() != 0));
    assert_eq!(pixels.data(), raster(&reference)?.data());
    Ok(())
}

fn raster(tree: &usvg::Tree) -> TestResult<tiny_skia::Pixmap> {
    let size = tree.size().to_int_size();
    let mut pixels =
        tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("pixmap allocation")?;
    crate::svg_engine::resvg::render(tree, Transform::identity(), &mut pixels.as_mut());
    Ok(pixels)
}

fn fixture() -> TestResult<(usvg::TextSpan, ResolvedFont)> {
    let tree = parse("text-decoration='underline'", "H")?;
    let span = text(&tree)?
        .chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .cloned()
        .ok_or("text span missing")?;
    let (database, id) = database_with_font(FONT.to_vec())?;
    let font = database.load_font(id, &[]).ok_or("font metrics missing")?;
    Ok((span, font))
}

#[rstest]
#[case::zero(0.0, false)]
#[case::negative(-1.0, false)]
#[case::nan(f32::NAN, false)]
#[case::infinite(f32::INFINITY, false)]
#[case::invalid_transform(VALID_DECORATION_WIDTH, true)]
fn malformed_decoration_spans_do_not_hide_valid_spans(
    #[case] width: f32,
    #[case] invalid_transform: bool,
) -> TestResult<()> {
    let (span, font) = fixture()?;
    let invalid = DecorationSpan {
        width,
        transform: if invalid_transform {
            Transform::from_translate(f32::NAN, 0.0)
        } else {
            Transform::identity()
        },
    };
    assert_decoration_isolated(&span, &font, invalid)
}

fn assert_decoration_isolated(
    span: &usvg::TextSpan,
    font: &ResolvedFont,
    invalid: DecorationSpan,
) -> TestResult<()> {
    let style = span
        .decoration()
        .underline()
        .cloned()
        .ok_or("underline missing")?;
    let valid = DecorationSpan {
        width: VALID_DECORATION_WIDTH,
        transform: Transform::identity(),
    };
    let convert = |spans: &[DecorationSpan]| {
        convert_decoration(0.0, span, font, style.clone(), spans, Transform::identity())
    };
    let expected = convert(&[valid]).ok_or("valid decoration missing")?;
    assert!(expected.is_visible() && expected.fill().is_some());
    assert!(convert(&[invalid]).is_none());
    let actual = convert(&[invalid, valid]).ok_or("valid decoration was discarded")?;
    assert_eq!(actual.bounding_box(), expected.bounding_box());
    assert!(actual.is_visible() && actual.fill().is_some());
    Ok(())
}

#[rstest]
#[case::missing_os2(b"OS/2", b"OS/3", assert_missing_strikeout)]
#[case::missing_post(b"post", b"posu", assert_missing_underline)]
fn missing_font_table_uses_decoration_fallback(
    #[case] original: &[u8; SFNT_TAG_SIZE],
    #[case] replacement: &[u8; SFNT_TAG_SIZE],
    #[case] check: fn(
        &ResolvedFont,
        &skrifa::metrics::Metrics,
        &skrifa::metrics::Metrics,
    ) -> TestResult<()>,
) -> TestResult<()> {
    use skrifa::MetadataProvider;
    let bytes = font_with_hidden_table(original, replacement)?;
    let source = skrifa::FontRef::from_index(FONT, 0)?;
    let modified = skrifa::FontRef::from_index(&bytes, 0)?;
    let before_location = source.axes().location(std::iter::empty::<(&str, f32)>());
    let after_location = modified.axes().location(std::iter::empty::<(&str, f32)>());
    let before = source.metrics(skrifa::prelude::Size::unscaled(), &before_location);
    let after = modified.metrics(skrifa::prelude::Size::unscaled(), &after_location);
    assert_eq!(after.units_per_em, before.units_per_em);
    let (database, id) = database_with_font(bytes)?;
    let resolved = database.load_font(id, &[]).ok_or("font metrics missing")?;
    check(&resolved, &before, &after)
}

#[test]
fn missing_x_height_uses_real_glyph_bounds() -> TestResult<()> {
    use read_fonts::TableProvider;
    use skrifa::MetadataProvider;

    let font = super::requested_font()?;
    let (mut normal, normal_id) = database_with_font(FONT.to_vec())?;
    super::assert_font_metrics_accepted(&mut normal, normal_id, &font)?;
    let bytes = font_with_hidden_table(b"OS/2", b"OS/3")?;
    assert!(read_fonts::FontRef::new(&bytes)?.os2().is_err());
    let source = skrifa::FontRef::from_index(&bytes, 0)?;
    let location = source.axes().location(std::iter::empty::<(&str, f32)>());
    assert!(
        source
            .metrics(skrifa::prelude::Size::unscaled(), &location)
            .x_height
            .is_none()
    );
    let expected = glyph_x_height_oracle(&bytes)?;
    let (mut database, id) = database_with_font(bytes)?;
    let resolver = usvg::FontResolver::default();
    assert_eq!((resolver.select_font)(&font, &mut database), Some(id));
    let actual = usvg::layout::css_font_x_height(&font, &resolver, &mut database)
        .ok_or("glyph x-height missing")?;
    assert!((actual - expected.0).abs() <= expected.1);
    Ok(())
}

fn glyph_x_height_oracle(bytes: &[u8]) -> TestResult<(f32, f32)> {
    let face = rustybuzz::ttf_parser::Face::parse(bytes, 0)?;
    let glyph = face.glyph_index('x').ok_or("x glyph missing")?;
    let bounds = face
        .glyph_bounding_box(glyph)
        .ok_or("x glyph bounds missing")?;
    assert!(bounds.y_max > 0);
    let units = f32::from(face.units_per_em());
    Ok((f32::from(bounds.y_max) / units, 1.0 / units))
}

fn assert_missing_underline(
    font: &ResolvedFont,
    before: &skrifa::metrics::Metrics,
    after: &skrifa::metrics::Metrics,
) -> TestResult<()> {
    assert!(before.underline.is_some() && after.underline.is_none());
    assert_eq!(
        font.underline_position(f32::from(after.units_per_em)),
        -f32::from(after.units_per_em / 9)
    );
    assert_missing_underline_thickness(font, after.units_per_em)?;
    Ok(())
}

fn assert_missing_strikeout(
    font: &ResolvedFont,
    before: &skrifa::metrics::Metrics,
    after: &skrifa::metrics::Metrics,
) -> TestResult<()> {
    assert!(before.strikeout.is_some() && after.strikeout.is_none());
    let height = after
        .x_height
        .filter(|height| *height > 0.0)
        .unwrap_or_else(|| {
            ((after.ascent - after.descent) * STRIKEOUT_FALLBACK_HEIGHT_RATIO).round()
        });
    assert!(height > 0.0 && height <= f32::from(i16::MAX));
    assert_eq!(
        font.line_through_position(f32::from(after.units_per_em)),
        (height.round() / 2.0).floor()
    );
    Ok(())
}

fn font_with_hidden_table(
    original: &[u8; SFNT_TAG_SIZE],
    replacement: &[u8; SFNT_TAG_SIZE],
) -> TestResult<Vec<u8>> {
    use read_fonts::FontRef;
    /* WHY: 必須テーブルが欠損した不正フォントでも代替値の耐性を確認する。 */
    let font = FontRef::new(FONT)?;
    let index = font
        .table_directory()
        .table_records()
        .iter()
        .position(|record| record.tag().to_be_bytes() == *original)
        .ok_or("font table missing")?;
    let offset = SFNT_DIRECTORY_OFFSET + SFNT_RECORD_SIZE * index;
    let mut bytes = FONT.to_vec();
    assert_eq!(&bytes[offset..offset + SFNT_TAG_SIZE], original);
    bytes[offset..offset + SFNT_TAG_SIZE].copy_from_slice(replacement);
    assert_eq!(&bytes[..offset], &FONT[..offset]);
    assert_eq!(
        &bytes[offset + SFNT_TAG_SIZE..],
        &FONT[offset + SFNT_TAG_SIZE..]
    );
    Ok(bytes)
}

fn assert_missing_underline_thickness(font: &ResolvedFont, units: u16) -> TestResult<()> {
    let (span, _) = fixture()?;
    let style = span
        .decoration()
        .underline()
        .cloned()
        .ok_or("underline missing")?;
    let path = convert_decoration(
        0.0,
        &span,
        font,
        style,
        &[DecorationSpan {
            width: VALID_DECORATION_WIDTH,
            transform: Transform::identity(),
        }],
        Transform::identity(),
    )
    .ok_or("decoration missing")?;
    let expected = f32::from(units / UNDERLINE_FALLBACK_THICKNESS_DIVISOR) * span.font_size().get()
        / f32::from(units);
    super::near(path.bounding_box().height(), expected);
    Ok(())
}
