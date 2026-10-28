use crate::markdown::diagram_js_runtime::{DiagramRuntimeScript, DiagramV8Runtime};

#[test]
fn fragment_bridge_preserves_five_argument_svg_metrics_and_rejects_invalid_inputs()
-> Result<(), String> {
    let script = r#"(() => {
      const old = globalThis.__katanaMeasureSvgTextMetrics('', 'sans-serif', 0, 400, false);
      const fragment = globalThis.__katanaMeasureSvgFragmentLayout;
      const empty = fragment([], 'sans-serif', 16, 400, false);
      const emptyPart = fragment([''], 'sans-serif', 16, 400, false);
      const parts = ['A', 'V'];
      parts.toString = () => { throw new Error('unexpected array coercion'); };
      const measured = fragment(parts, 'sans-serif', 16, 400, false);
      const noArrayCoercion = measured === null ||
        (typeof measured === 'object' && Array.isArray(measured.advances));
      let invalid = false;
      try { fragment('not an array', 'sans-serif', 16, 400, false); }
      catch (error) { invalid = error instanceof Error; }
      return JSON.stringify({ old: old.advance, empty, emptyPart, invalid, noArrayCoercion });
    })()"#;
    let observed = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
        "svg-fragment-bridge-boundary.js",
        script,
    )])?;
    let actual: serde_json::Value = serde_json::from_str(&observed).map_err(|e| e.to_string())?;
    assert_eq!(
        actual,
        serde_json::json!({"old": 0, "empty": null, "emptyPart": null, "invalid": true, "noArrayCoercion": true})
    );
    Ok(())
}

const FRAGMENT_LIMIT_SCRIPT: &str = r#"(() => {
      const measure = globalThis.__katanaMeasureSvgFragmentLayout;
      const call = (parts, size = 16) => measure(parts, 'sans-serif', size, 400, false);
      const atLimit = Array(4096).fill('');
      const overLimit = Array(4097).fill('');
      let checkedAtLimit = false;
      Object.defineProperty(atLimit, 4095, { get() { checkedAtLimit = true; return ''; } });
      const atResult = call(atLimit);
      let checkedOverLimit = false;
      Object.defineProperty(overLimit, 4096, { configurable: true,
        get() { checkedOverLimit = true; return ''; } });
      const overResult = call(overLimit);
      Object.defineProperty(overLimit, 4096, { configurable: true, value: 1 });
      let invalidPart = false;
      try { call(overLimit); } catch (error) { invalidPart = error instanceof Error; }
      Object.defineProperty(overLimit, 4096, { configurable: true, value: '' });
      let invalidSize = false;
      try { call(overLimit, -1); } catch (error) { invalidSize = error instanceof Error; }
      const old = globalThis.__katanaMeasureSvgTextMetrics('', 'sans-serif', 0, 400, false);
      return JSON.stringify({ checkedAtLimit, checkedOverLimit, atResult, overResult, invalidPart, invalidSize,
        old: old.advance });
    })()"#;

#[test]
fn fragment_bridge_limit_falls_back_only_for_valid_requests() -> Result<(), String> {
    let observed = DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
        "svg-fragment-limit.js",
        FRAGMENT_LIMIT_SCRIPT,
    )])?;
    let actual: serde_json::Value = serde_json::from_str(&observed).map_err(|e| e.to_string())?;
    assert_eq!(
        actual,
        serde_json::json!({"checkedAtLimit": true, "checkedOverLimit": true, "atResult": null,
        "overResult": null, "invalidPart": true, "invalidSize": true, "old": 0})
    );
    Ok(())
}
