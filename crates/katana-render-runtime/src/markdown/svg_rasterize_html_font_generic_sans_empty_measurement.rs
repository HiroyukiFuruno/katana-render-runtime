use super::super::super::super::{DiscoveryMode, HtmlFontRequest, decision_observer};
use super::super::TEXT;
use crate::markdown::svg_rasterize::{SvgRasterizeOps, text_shaping};

const FONT_SIZE_PX: f32 = 16.0;
const LINE_HEIGHT_MULTIPLIER: f32 = 1.25;

type EmptyRunsObservation = (Vec<(String, String)>, Vec<decision_observer::Decision>);

fn assert_empty_record(
    record: &decision_observer::Decision,
    family: &str,
    weight: u16,
    italic: bool,
) {
    assert_eq!(record.family, family);
    assert!(record.text.is_empty());
    assert_eq!(record.weight, weight);
    assert_eq!(record.italic, italic);
    assert_eq!(record.key.request, HtmlFontRequest::from_text(family, ""));
    assert_eq!(record.key.discovery, DiscoveryMode::KnownOnly);
}

fn assert_empty_width(
    observer: &decision_observer::Scope,
    family: &str,
    weight: u16,
    italic: bool,
    expected_records: &mut usize,
) -> Vec<decision_observer::Decision> {
    let width = SvgRasterizeOps::with_html_font_render_scope(|| {
        text_shaping::shaped_text_width(
            "",
            family,
            FONT_SIZE_PX,
            weight,
            italic,
            LINE_HEIGHT_MULTIPLIER,
            None,
        )
    });
    assert_eq!(
        width,
        Some(0.0),
        "empty shaped width for {family}/{weight}/{italic}"
    );
    let after_width = observer.records();
    assert_eq!(after_width.len(), *expected_records + 1);
    let width_record = &after_width[*expected_records];
    assert_empty_record(width_record, family, weight, italic);
    *expected_records += 1;
    after_width
}

fn assert_empty_runs(
    observer: &decision_observer::Scope,
    family: &str,
    weight: u16,
    italic: bool,
    expected_records: &mut usize,
) -> Result<EmptyRunsObservation, String> {
    let runs = SvgRasterizeOps::with_html_font_render_scope(|| {
        SvgRasterizeOps::html_text_font_runs("", family, weight, italic)
    })
    .ok_or("empty font-run lookup returned no face")?;
    assert!(runs.is_empty(), "empty text must produce no font runs");
    let after_runs = observer.records();
    assert_eq!(after_runs.len(), *expected_records + 1);
    let runs_record = &after_runs[*expected_records];
    assert_empty_record(runs_record, family, weight, italic);
    *expected_records += 1;
    Ok((runs, after_runs))
}

fn assert_empty_line_metrics(
    observer: &decision_observer::Scope,
    family: &str,
    weight: u16,
    italic: bool,
) {
    let records_before_line_metrics = observer.records().len();
    let line_metrics = SvgRasterizeOps::with_html_font_render_scope(|| {
        SvgRasterizeOps::html_line_metrics("", family, FONT_SIZE_PX, weight, italic)
    });
    assert!(line_metrics.is_none(), "empty line metrics remain absent");
    assert_eq!(observer.records().len(), records_before_line_metrics);
}

pub(in super::super) fn assert_empty_measurement_pair(
    observer: &decision_observer::Scope,
    family: &str,
    weight: u16,
    italic: bool,
    expected_records: &mut usize,
) -> Result<(), String> {
    let _after_width = assert_empty_width(observer, family, weight, italic, expected_records);
    let (_runs, _after_runs) =
        assert_empty_runs(observer, family, weight, italic, expected_records)?;
    assert_empty_line_metrics(observer, family, weight, italic);
    Ok(())
}

pub(in super::super) fn assert_nonempty_latin_metrics(
    family: &str,
    weight: u16,
    italic: bool,
) -> Result<(), String> {
    let (space, text) = SvgRasterizeOps::with_html_font_render_scope(|| {
        (
            SvgRasterizeOps::html_line_metrics(" ", family, FONT_SIZE_PX, weight, italic),
            SvgRasterizeOps::html_line_metrics(TEXT, family, FONT_SIZE_PX, weight, italic),
        )
    });
    let space = space.ok_or("nonempty space lost real font metrics")?;
    assert_eq!(Some(space), text);
    assert!(space.0.is_finite() && space.1.is_finite() && space.0 + space.1 > 0.0);
    Ok(())
}
