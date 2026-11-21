use super::super::super::HtmlGlyphRasterizer;
use resvg::usvg::fontdb::{Database, ID, Weight};
use skia_safe::{Font, FontHinting, FontMgr, Paint, Typeface};
use skrifa::{FontRef, MetadataProvider, prelude::Size};

#[path = "svg_rasterize_html_glyph_font_baseline_tests.rs"]
mod font_baseline_tests;

const FAMILIES: [&str; 2] = ["Helvetica", "Trebuchet MS"];
const WEIGHTS: [Weight; 2] = [Weight::NORMAL, Weight::BOLD];
const SIZES: [f32; 4] = [12.0, 14.0, 16.0, 18.0];
const METRIC_COUNT: usize = 7;
const METRIC_TOLERANCE: f32 = 0.0001;
const USVG_X_HEIGHT_RATIO: f32 = 0.45;
const HALF_METRIC: f32 = 0.5;
const METRIC_SET_COUNT: usize = 3;

#[derive(Clone, Copy)]
struct MetricSet {
    size: f32,
    values: [f32; METRIC_COUNT],
    x_glyph_height: Option<f32>,
}

#[test]
fn font_metrics_are_finite_for_file_and_coretext_faces() -> Result<(), String> {
    for family in FAMILIES {
        let family_database = super::super::super::super::font::html_font_db_for_text(family, "AV");
        let family_rasterizer = HtmlGlyphRasterizer::new(&family_database);
        for weight in WEIGHTS {
            compare_font_metrics(&family_database, &family_rasterizer, family, weight)?;
        }
    }
    Ok(())
}

fn compare_font_metrics(
    database: &Database,
    rasterizer: &HtmlGlyphRasterizer<'_>,
    family: &str,
    weight: Weight,
) -> Result<(), String> {
    let id = super::super::find_face(database, family, weight)?;
    let source = super::source_face(database, rasterizer, id)?;
    let (cached, cached_data, cached_index) = super::cached_source(rasterizer, id)?;
    let expected = if source.index == 0 {
        source.bytes.clone()
    } else {
        super::repacked_face(&source.bytes, source.index)?.0
    };
    super::assert_cached_binding(&source, &cached_data, cached_index, &expected)?;
    let system = FontMgr::default()
        .legacy_make_typeface(Some(family), cached.font_style())
        .ok_or("CoreText typeface unavailable")?;
    let skrifa = skrifa_metrics(database, id)?;
    for size in SIZES {
        report_metrics(family, weight, &cached, &system, &skrifa, size)?;
    }
    Ok(())
}

fn skrifa_metrics(database: &Database, id: ID) -> Result<MetricSet, String> {
    let values = database
        .with_face_data(id, |bytes, index| {
            let font = FontRef::from_index(bytes, index).ok()?;
            let location = font.axes().location(std::iter::empty::<(&str, f32)>());
            let metrics = font.metrics(Size::unscaled(), &location);
            Some((metrics, metrics.units_per_em as f32))
        })
        .flatten()
        .ok_or("skrifa metrics unavailable")?;
    let (metrics, upem) = values;
    Ok(skrifa_metric_set(
        metrics.ascent,
        metrics.descent,
        metrics.x_height,
        metrics.cap_height,
        upem,
    ))
}

fn skrifa_metric_set(
    ascent_value: f32,
    descent_value: f32,
    x_height_value: Option<f32>,
    cap_height_value: Option<f32>,
    upem: f32,
) -> MetricSet {
    let ascent = ascent_value.round();
    let descent = descent_value.round();
    let x_height = x_height_value
        .filter(|value| *value > 0.0)
        /* WHY: 欠落時もusvgと同じascent/descent幅の45%を使う。 */
        .unwrap_or((ascent_value - descent_value) * USVG_X_HEIGHT_RATIO)
        .round();
    let cap_height = cap_height_value.unwrap_or(ascent_value).round();
    MetricSet {
        size: upem,
        values: metric_values(ascent, descent, x_height, cap_height),
        x_glyph_height: None,
    }
}

fn report_metrics(
    family: &str,
    weight: Weight,
    cached: &Typeface,
    system: &Typeface,
    skrifa: &MetricSet,
    size: f32,
) -> Result<(), String> {
    let cached_metrics = skia_metrics(cached, size);
    let system_metrics = skia_metrics(system, size);
    let skrifa_metrics = scaled_skrifa(*skrifa, size);
    assert_finite([cached_metrics, system_metrics, skrifa_metrics])?;
    assert_metric_close(cached_metrics, system_metrics)?;
    eprintln!(
        "font_metrics family={family} weight={} size={size:.1} cached={:?}/{:?} system={:?}/{:?} skrifa={:?}/{:?}",
        weight.0,
        cached_metrics.values,
        cached_metrics.x_glyph_height,
        system_metrics.values,
        system_metrics.x_glyph_height,
        skrifa_metrics.values,
        skrifa_metrics.x_glyph_height,
    );
    Ok(())
}

fn skia_metrics(typeface: &Typeface, size: f32) -> MetricSet {
    let font = diagnostic_font(typeface, size);
    let (_, metrics) = font.metrics();
    let mut paint = Paint::default();
    paint.set_argb(super::super::OPAQUE_ALPHA, 0, 0, 0);
    let x_height = font.measure_str("x", Some(&paint)).1.height();
    let values = metric_values(
        -metrics.ascent,
        -metrics.descent,
        metrics.x_height,
        metrics.cap_height,
    );
    MetricSet {
        size,
        values,
        x_glyph_height: Some(x_height),
    }
}

fn metric_values(ascent: f32, descent: f32, x_height: f32, cap_height: f32) -> [f32; METRIC_COUNT] {
    [
        ascent,
        descent,
        x_height,
        cap_height,
        ascent * HALF_METRIC,
        x_height * HALF_METRIC,
        (ascent + descent) * HALF_METRIC,
    ]
}

fn diagnostic_font(typeface: &Typeface, size: f32) -> Font {
    let mut font = Font::new(typeface.clone(), size);
    font.set_subpixel(true)
        .set_linear_metrics(true)
        .set_baseline_snap(true)
        .set_hinting(FontHinting::Normal)
        .set_edging(skia_safe::font::Edging::SubpixelAntiAlias);
    font
}

fn scaled_skrifa(metrics: MetricSet, size: f32) -> MetricSet {
    let scale = size / metrics.size;
    MetricSet {
        size,
        values: metrics.values.map(|value| value * scale),
        x_glyph_height: metrics.x_glyph_height.map(|value| value * scale),
    }
}

fn assert_finite(sets: [MetricSet; METRIC_SET_COUNT]) -> Result<(), String> {
    if sets.iter().flat_map(|set| set.values).all(f32::is_finite)
        && sets.iter().all(|set| {
            set.x_glyph_height
                .map(|value| value.is_finite())
                .unwrap_or(true)
        })
    {
        Ok(())
    } else {
        Err("font metrics contained a non-finite value".to_string())
    }
}

fn assert_metric_close(cached: MetricSet, system: MetricSet) -> Result<(), String> {
    if cached
        .values
        .iter()
        .zip(system.values)
        .all(|(file, coretext)| (file - coretext).abs() <= METRIC_TOLERANCE)
        && match (cached.x_glyph_height, system.x_glyph_height) {
            (Some(file), Some(coretext)) => (file - coretext).abs() <= METRIC_TOLERANCE,
            _ => true,
        }
    {
        Ok(())
    } else {
        Err("cached and CoreText metrics differ beyond tolerance".to_string())
    }
}
