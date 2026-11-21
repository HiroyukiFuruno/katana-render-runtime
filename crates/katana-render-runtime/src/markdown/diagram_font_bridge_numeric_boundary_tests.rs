use crate::markdown::diagram_js_runtime::{DiagramRuntimeScript, DiagramV8Runtime};

const INVALID_NUMERIC_REQUEST_SCRIPT: &str = r#"(() => {
    const css = globalThis.__katanaMeasureText;
    const svg = globalThis.__katanaMeasureSvgTextMetrics;
    const cases = [
        ['nan-size', NaN, 400], ['infinite-size', Infinity, 400],
        ['negative-size', -1, 400], ['nonnumeric-string-size', 'sixteen', 400],
        ['negative-weight', 16, 0], ['high-weight', 16, 1001],
        ['infinite-weight', 16, Infinity], ['nonnumeric-string-weight', 16, 'four hundred'],
    ];
    return cases.map(([name, size, weight]) => {
        let svgRejected = false;
        try { svg('A', 'Noto Sans', size, weight, false); }
        catch (error) {
            svgRejected = error instanceof Error &&
                error.message === 'Invalid SVG font metric request';
        }
        return [name, css('A', 'Noto Sans', size, weight, false) === undefined,
            svgRejected].join(':');
    }).join(',');
})()"#;

#[test]
fn real_v8_numeric_request_table_rejects_invalid_size_and_weight() -> Result<(), String> {
    let result = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
        "bridge-invalid-numeric-request-table",
        INVALID_NUMERIC_REQUEST_SCRIPT,
    )])?;
    assert_eq!(
        result,
        "nan-size:true:true,infinite-size:true:true,negative-size:true:true,\
         nonnumeric-string-size:true:true,negative-weight:true:true,high-weight:true:true,\
         infinite-weight:true:true,nonnumeric-string-weight:true:true"
    );
    Ok(())
}
