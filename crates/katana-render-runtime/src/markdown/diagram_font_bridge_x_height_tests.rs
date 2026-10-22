use crate::markdown::diagram_js_runtime::{DiagramRuntimeScript, DiagramV8Runtime};
use crate::markdown::svg_rasterize::SvgRasterizeOps;

#[test]
fn real_v8_x_height_bridge_returns_native_font_metric_for_real_font() -> Result<(), String> {
    let expected = SvgRasterizeOps::measure_svg_font_x_height("Noto Sans", 16.0, 400, false)
        .map_err(|error| format!("real Noto Sans x-height unavailable: {error:?}"))?
        .ok_or("real Noto Sans face must declare its x-height")?;
    let script = r#"(() => {
      const measure = globalThis.__katanaMeasureSvgFontXHeight;
      const value = measure('Noto Sans', 16, 400, false);
      let invalidSize = false;
      try { measure('Noto Sans', 0, 400, false); }
      catch (error) { invalidSize = error.message === 'Invalid SVG font x-height request'; }
      let invalidWeight = false;
      try { measure('Noto Sans', 16, 1001, false); }
      catch (error) { invalidWeight = error.message === 'Invalid SVG font x-height request'; }
      return [value, invalidSize, invalidWeight].join(',');
    })()"#;
    let observed = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
        "bridge-real-svg-font-x-height",
        script,
    )])?;
    assert_bridge_x_height_result(&observed, expected)
}

#[cfg(target_os = "macos")]
#[test]
fn real_v8_bridge_measures_trebuchet_x_glyph_when_table_x_height_is_missing() -> Result<(), String>
{
    let script = r#"(() => {
      const measure = globalThis.__katanaMeasureSvgFontXHeight;
      return [measure('Trebuchet MS', 16, 400, false), measure('Trebuchet MS', 14, 700, false)].join(',');
    })()"#;
    let observed = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
        "bridge-trebuchet-x-height-from-glyph-extents",
        script,
    )])?;
    assert_eq!(observed, "8.3671875,7.3212890625");
    Ok(())
}

fn assert_bridge_x_height_result(observed: &str, expected: f32) -> Result<(), String> {
    let mut fields = observed.split(',');
    let actual = fields
        .next()
        .ok_or("V8 x-height result missing")?
        .parse::<f32>()
        .map_err(|error| format!("invalid V8 x-height result: {error}"))?;
    assert_eq!(actual, expected);
    assert_eq!(fields.next(), Some("true"));
    assert_eq!(fields.next(), Some("true"));
    assert_eq!(fields.next(), None);
    Ok(())
}
