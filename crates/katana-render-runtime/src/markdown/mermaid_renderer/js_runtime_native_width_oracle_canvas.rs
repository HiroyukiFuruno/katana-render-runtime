use super::call::ParsedCall;
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use serde_json::Value;

const CANVAS_SEGMENTS_ARGUMENT_INDEX: usize = 5;

pub(super) fn assert_call(call: &Value, parsed: ParsedCall<'_>) -> Result<(), String> {
    let Some(segments) = canvas_segments(call)? else {
        return assert_single_canvas_call(parsed);
    };
    assert_segmented_canvas_call(segments, parsed)
}

fn assert_single_canvas_call(parsed: ParsedCall<'_>) -> Result<(), String> {
    let expected = measure_html_text(&parsed, parsed.text)?;
    assert_eq!(
        parsed.actual,
        f64::from(expected),
        "Canvas {:?}",
        parsed.text
    );
    Ok(())
}

fn assert_segmented_canvas_call(segments: &[Value], parsed: ParsedCall<'_>) -> Result<(), String> {
    let mut text = String::new();
    let mut expected = 0.0_f64;
    for segment in segments {
        let segment = segment.as_str().ok_or("Canvas segment is not text")?;
        text.push_str(segment);
        expected += f64::from(measure_html_text(&parsed, segment)?);
    }
    assert_eq!(
        text, parsed.text,
        "Canvas segments changed the original text"
    );
    assert_eq!(parsed.actual, expected, "Canvas {:?}", parsed.text);
    Ok(())
}

fn measure_html_text(parsed: &ParsedCall<'_>, text: &str) -> Result<f32, String> {
    SvgRasterizeOps::measure_html_text(
        text,
        parsed.family,
        parsed.size,
        parsed.weight,
        parsed.italic,
        0.0,
        None,
    )
    .map_err(|error| format!("Canvas boundary reference failed: {error:?}"))
}

fn canvas_segments(call: &Value) -> Result<Option<&[Value]>, String> {
    let args = call
        .get("args")
        .and_then(Value::as_array)
        .ok_or("Canvas arguments missing")?;
    let Some(segments) = args.get(CANVAS_SEGMENTS_ARGUMENT_INDEX) else {
        return Ok(None);
    };
    segments
        .as_array()
        .map(Vec::as_slice)
        .map(Some)
        .ok_or_else(|| "Canvas segments missing".to_string())
}
