const C4_WEIGHT: u16 = 400;
const CSS_SUBPIXELS: f64 = 64.0;

fn assert_c4_native_width_rows(
    cases: &[(&str, &str, f32)],
    rows: &[serde_json::Value],
) -> Result<(), String> {
    assert!(!cases.is_empty());
    assert_eq!(rows.len(), cases.len());
    let mut has_rounding_counterexample = false;
    let mut has_css_rounding_counterexample = false;
    for (row, &(text, family, size)) in rows.iter().zip(cases) {
        let (svg_counterexample, css_counterexample) =
            assert_c4_native_width_row(row, text, family, size)?;
        has_rounding_counterexample |= svg_counterexample;
        has_css_rounding_counterexample |= css_counterexample;
    }
    assert!(
        has_rounding_counterexample,
        "SVG inputs must distinguish raw advance from CSS ceil64"
    );
    assert!(
        has_css_rounding_counterexample,
        "CSS inputs must distinguish ceil64 from raw advance"
    );
    Ok(())
}

fn assert_c4_native_width_row(
    row: &serde_json::Value,
    text: &str,
    family: &str,
    size: f32,
) -> Result<(bool, bool), String> {
    let calls = row["calls"].as_array().ok_or("native calls missing")?;
    assert_eq!(calls.len(), 2, "{text:?}: {calls:?}");
    assert_c4_native_call_arguments(calls, text, family, size)?;
    let svg_advance = assert_c4_native_svg_reference(&calls[0], row, text, family, size)?;
    let css_raw =
        SvgRasterizeOps::measure_html_text(text, family, size, C4_WEIGHT, false, 0.0, None)
            .map_err(|error| format!("CSS {text:?}, {family}: {error:?}"))?;
    assert!(css_raw.is_finite() && css_raw > 1.0);
    let css_expected = (f64::from(css_raw) * CSS_SUBPIXELS).ceil() / CSS_SUBPIXELS;
    assert_eq!(calls[1]["width"].as_f64(), Some(css_expected), "{text:?}");
    assert_eq!(row["cssWidth"].as_f64(), Some(css_expected), "{text:?}");
    let rounded_svg = (f64::from(svg_advance) * CSS_SUBPIXELS).ceil() / CSS_SUBPIXELS;
    Ok((
        rounded_svg as f32 != svg_advance,
        f64::from(css_raw) != css_expected,
    ))
}

fn assert_c4_native_svg_reference(
    call: &serde_json::Value,
    row: &serde_json::Value,
    text: &str,
    family: &str,
    size: f32,
) -> Result<f32, String> {
    let svg =
        SvgRasterizeOps::measure_svg_text_metrics(text, family, size, C4_WEIGHT, false)
            .map_err(|error| format!("SVG {text:?}, {family}: {error:?}"))?;
    assert!(
        svg.advance.is_finite()
            && svg.advance > 1.0
            && svg.ascent.is_finite()
            && svg.descent.is_finite()
            && svg.ascent + svg.descent > 0.0,
        "nonzero native SVG reference required: {text:?}, {family}"
    );
    assert_c4_svg_metrics(call, row, &svg, text, family)?;
    Ok(svg.advance)
}

fn assert_c4_native_call_arguments(
    calls: &[serde_json::Value],
    text: &str,
    family: &str,
    size: f32,
) -> Result<(), String> {
    for (call, kind) in calls.iter().zip(["svg", "css"]) {
        assert_eq!(call["kind"].as_str(), Some(kind), "{text:?}");
        let args = call["args"].as_array().ok_or("arguments missing")?;
        assert_eq!(args.len(), 5, "{text:?}");
        assert_eq!(args[0].as_str(), Some(text));
        assert_eq!(args[1].as_str(), Some(family));
        assert_eq!(args[2].as_f64(), Some(f64::from(size)));
        assert_eq!(args[3].as_f64(), Some(f64::from(C4_WEIGHT)));
        assert_eq!(args[4].as_bool(), Some(false));
    }
    Ok(())
}

fn assert_c4_svg_metrics(
    call: &serde_json::Value,
    row: &serde_json::Value,
    expected: &crate::markdown::svg_rasterize::SvgTextRunMetrics,
    text: &str,
    family: &str,
) -> Result<(), String> {
    for (name, metric) in [
        ("advance", expected.advance),
        ("ascent", expected.ascent),
        ("descent", expected.descent),
    ] {
        assert_eq!(
            c4_native_f32(&call["metrics"][name])?,
            metric,
            "SVG {name}, {text:?}, {family}"
        );
    }
    /* WHY: callbackのraw値を維持し、DOM APIの幅だけChromeと同じ1/64pxへ切り上げる。 */
    let expected_dom_width =
        (f64::from(expected.advance) * CSS_SUBPIXELS).ceil() / CSS_SUBPIXELS;
    assert_eq!(row["width"].as_f64(), Some(expected_dom_width), "{text:?}");
    Ok(())
}
