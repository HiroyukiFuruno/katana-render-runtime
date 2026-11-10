use crate::markdown::diagram_js_runtime::{DiagramRuntimeScript, DiagramV8Runtime};

#[test]
fn real_svg_bridge_distinguishes_true_zero_from_positive_f64_underflow() -> Result<(), String> {
    use crate::markdown::svg_rasterize::SvgRasterizeOps;

    let zero = SvgRasterizeOps::measure_svg_text_metrics("DB", "Noto Sans", 0.0, 400, false)
        .map_err(|error| format!("true zero SVG request failed: {error:?}"))?;
    assert_eq!([zero.advance, zero.ascent, zero.descent], [0.0; 3]);
    let script = r#"(() => {
        const svg = globalThis.__katanaMeasureSvgTextMetrics;
        const css = globalThis.__katanaMeasureText;
        const normal = svg('DB', 'Noto Sans', 16, 400, false);
        const zero = svg('DB', 'Noto Sans', 0, 400, false);
        const tiny = 1e-100;
        let rejected = false;
        try { svg('DB', 'Noto Sans', tiny, 400, false); }
        catch (error) { rejected = error instanceof Error; }
        return [normal.advance > 0 && normal.ascent + normal.descent > 0,
            [zero.advance, zero.ascent, zero.descent].every(value => value === 0),
            tiny > 0 && Math.fround(tiny) === 0,
            rejected, css('DB', 'Noto Sans', tiny, 400, false) === undefined,
            css('DB', 'Noto Sans', 0, 400, false) === undefined].join(',');
    })()"#;
    let result = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
        "bridge-svg-zero-positive-underflow",
        script,
    )])?;
    assert_eq!(result, "true,true,true,true,true,true");
    Ok(())
}

#[test]
fn finite_size_above_shaping_range_is_rejected_by_both_svg_font_callbacks() -> Result<(), String> {
    let script = r#"(() => {
        const measure = globalThis.__katanaMeasureSvgTextMetrics;
        const xHeight = globalThis.__katanaMeasureSvgFontXHeight;
        const size = 1e8;
        const checks = [
            [() => measure('DB', 'Noto Sans', size, 400, false), 'SVG font metric measurement failed'],
            [() => xHeight('Noto Sans', size, 400, false), 'SVG font x-height measurement failed'],
        ];
        return Number.isFinite(Math.fround(size)) && checks.every(([call, expected]) => {
            try { call(); }
            catch (error) { return error instanceof Error && error.message === expected; }
            return false;
        });
    })()"#;
    let result = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
        "bridge-svg-shaping-scale-range",
        script,
    )])?;
    assert_eq!(result, "true");
    Ok(())
}

#[test]
fn finite_javascript_size_outside_f32_request_range_returns_undefined() -> Result<(), String> {
    let script = r#"(() => {
        const measure = globalThis.__katanaMeasureText;
        const ordinary = measure('WWWW', 'Noto Sans', 16, 400, false);
        const large = 1e100;
        const rejected = measure('WWWW', 'Noto Sans', large, 400, false);
        return [Number.isFinite(ordinary) && ordinary > 0,
            Number.isFinite(large) && !Number.isFinite(Math.fround(large)),
            rejected === undefined].join(',');
    })()"#;
    let result = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
        "bridge-f32-request-range",
        script,
    )])?;
    assert_eq!(result, "true,true,true");
    Ok(())
}

#[test]
fn canvas_bridge_rejects_non_string_or_empty_segments() -> Result<(), String> {
    let script = r#"(() => {
        const measure = globalThis.__katanaMeasureCanvasText;
        const invalid = [
            measure('AB', 'Noto Sans', 16, 400, false, ['A', 1]),
            measure('AB', 'Noto Sans', 16, 400, false, ['A', '']),
            measure('AB', 'Noto Sans', 16, 400, false, ['A']),
        ];
        const empty = measure('', 'Noto Sans', 16, 400, false, []);
        return invalid.every(value => value === undefined) && empty === 0;
    })()"#;
    let result = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
        "bridge-canvas-segment-contract",
        script,
    )])?;
    assert_eq!(result, "true");
    Ok(())
}

#[test]
fn nul_text_with_empty_family_returns_parse_error_and_undefined() {
    use crate::markdown::svg_rasterize::{SvgRasterizeError, SvgRasterizeOps};

    let invalid = SvgRasterizeOps::measure_html_text("\0", "", 16.0, 400, false, 0.0, None);
    assert!(matches!(invalid, Err(SvgRasterizeError::ParseFailed(_))));
    assert!(matches!(
        SvgRasterizeOps::measure_html_text("A", "", 16.0, 400, false, 0.0, None),
        Ok(width) if width.is_finite() && width >= 0.0
    ));
    let script = r#"(() => {
        const measure = globalThis.__katanaMeasureText;
        return measure('\u0000', '', 16, 400, false) === undefined;
    })()"#;
    let result = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
        "bridge-nul-empty-family-measurement",
        script,
    )]);
    assert!(matches!(result.as_deref(), Ok("true")));
}

#[test]
fn canvas_font_measurement_recovers_after_invalid_requests() -> Result<(), String> {
    let script = r#"(() => {
        const measure = globalThis.__katanaMeasureCanvasText;
        const call = (text, family, ...segments) =>
            measure(text, family, 16, 400, false, ...segments);
        const first = call('A', 'Noto Sans');
        const second = call('B', 'Noto Sans');
        const combined = call('AB', 'Noto Sans', ['A', 'B']);
        const rejected = [
            call('\u0000', ''),
            call('A\u0000', '', ['A', '\u0000']),
            measure('A', 'Noto Sans', 0, 400, false),
            call('A', 'Noto Sans', ['A', 'B']),
        ];
        return [Number.isFinite(first) && first > 0,
            Number.isFinite(second) && second > 0,
            combined === first + second,
            rejected.every(value => value === undefined),
            call('A', 'Noto Sans') === first].join(',');
    })()"#;
    let result = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
        "bridge-canvas-recovery",
        script,
    )])?;
    assert_eq!(result, "true,true,true,true,true");
    Ok(())
}
