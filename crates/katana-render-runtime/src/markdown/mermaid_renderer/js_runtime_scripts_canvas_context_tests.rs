use super::render_svg_fixture;

#[test]
fn native_canvas_measure_text_uses_native_whole_string_advance() -> Result<(), String> {
    let rendered = render_svg_fixture(CANVAS_NATIVE_FIXTURE)?;
    assert!(
        rendered.contains(r##"data-width-equal="true" data-call="true,true,true,true,true""##),
        "{rendered}"
    );
    Ok(())
}

#[test]
fn c4_canvas_measure_text_keeps_legacy_fallback_width() -> Result<(), String> {
    let rendered = render_svg_fixture(CANVAS_C4_FIXTURE)?;
    assert!(
        rendered.contains(r##"data-fallback="true" data-native-called="false""##),
        "{rendered}"
    );
    Ok(())
}

#[test]
fn native_canvas_measure_text_propagates_bridge_exception() -> Result<(), String> {
    let rendered = render_svg_fixture(CANVAS_EXCEPTION_FIXTURE);
    assert!(
        rendered
            .as_ref()
            .is_err_and(|error| error.contains("bridge failure")),
        "{rendered:?}"
    );
    Ok(())
}

#[test]
fn native_canvas_measure_text_rejects_invalid_bridge_result() -> Result<(), String> {
    let rendered = render_svg_fixture(CANVAS_INVALID_FIXTURE);
    assert!(
        rendered
            .as_ref()
            .is_err_and(|error| error.contains("native Canvas font measurement is invalid")),
        "{rendered:?}"
    );
    Ok(())
}

const CANVAS_NATIVE_FIXTURE: &str = r##"
globalThis.mermaid = { initialize() {}, render: async () => {
  globalThis.__katanaMermaidDiagramType = 'flowchart';
  const original = globalThis.__katanaMeasureText;
  const calls = [];
  globalThis.__katanaMeasureText = (...args) => { calls.push(args); return original(...args); };
  const canvas = document.createElement('canvas').getContext('2d');
  canvas.font = 'normal italic bold 16px/normal "Helvetica Neue", Helvetica, sans-serif';
  const width = canvas.measureText('AV').width;
  const expected = original('AV', '"Helvetica Neue", Helvetica, sans-serif', 16, 700, true);
  const call = calls[0];
  return { svg: '<svg data-width-equal="' + (width === expected) + '" data-call="' + [call[0] === 'AV', call[1] === '"Helvetica Neue", Helvetica, sans-serif', call[2] === 16, call[3] === 700, call[4] === true].join(',') + '"></svg>' };
}};
"##;

const CANVAS_C4_FIXTURE: &str = r##"
globalThis.mermaid = { initialize() {}, render: async () => {
  globalThis.__katanaMermaidDiagramType = 'c4-context';
  let nativeCalled = false;
  globalThis.__katanaMeasureText = () => { nativeCalled = true; throw new Error('native bridge must not be called'); };
  const canvas = document.createElement('canvas').getContext('2d');
  canvas.font = '16px "Helvetica Neue", Helvetica, sans-serif';
  const width = canvas.measureText('A').width;
  return { svg: '<svg data-fallback="' + (width > 10.672 && width < 10.673) + '" data-native-called="' + nativeCalled + '"></svg>' };
}};
"##;

const CANVAS_EXCEPTION_FIXTURE: &str = r##"
globalThis.mermaid = { initialize() {}, render: async () => {
  globalThis.__katanaMermaidDiagramType = 'architecture';
  globalThis.__katanaMeasureText = () => { throw new Error('bridge failure'); };
  const canvas = document.createElement('canvas').getContext('2d');
  canvas.font = '16px Helvetica, sans-serif';
  canvas.measureText('AV');
  return { svg: '<svg></svg>' };
}};
"##;

const CANVAS_INVALID_FIXTURE: &str = r##"
globalThis.mermaid = { initialize() {}, render: async () => {
  globalThis.__katanaMermaidDiagramType = 'architecture';
  globalThis.__katanaMeasureText = () => undefined;
  const canvas = document.createElement('canvas').getContext('2d');
  canvas.font = '16px Helvetica, sans-serif';
  canvas.measureText('AV');
  return { svg: '<svg></svg>' };
}};
"##;
