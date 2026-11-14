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
fn native_canvas_measure_text_splits_cjk_runs_for_canvas_only() -> Result<(), String> {
    let rendered = render_svg_fixture(CANVAS_CJK_FIXTURE)?;
    assert!(
        rendered.contains(r##"data-segments="デー|タ|ベー|ス" data-width="true""##),
        "{rendered}"
    );
    Ok(())
}

#[test]
fn native_canvas_measure_text_preserves_special_run_boundaries() -> Result<(), String> {
    let rendered = render_svg_fixture(CANVAS_BOUNDARY_FIXTURE)?;
    assert!(
        rendered.contains(
            r##"data-ascii="true" data-leading="ース" data-punctuation="日|本|,abc" data-latin="résumé.test" data-mark="が" data-emoji="👩‍💻""##
        ),
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

#[test]
fn segmented_canvas_measure_text_propagates_bridge_exception() -> Result<(), String> {
    let rendered = render_svg_fixture(CANVAS_SEGMENTED_EXCEPTION_FIXTURE);
    assert!(
        rendered
            .as_ref()
            .is_err_and(|error| error.contains("segmented bridge failure")),
        "{rendered:?}"
    );
    Ok(())
}

#[test]
fn segmented_canvas_measure_text_rejects_invalid_bridge_result() -> Result<(), String> {
    let rendered = render_svg_fixture(CANVAS_SEGMENTED_INVALID_FIXTURE);
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
  const original = globalThis.__katanaMeasureCanvasText;
  const calls = [];
  globalThis.__katanaMeasureCanvasText = (...args) => { calls.push(args); return original(...args); };
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

const CANVAS_CJK_FIXTURE: &str = r##"
globalThis.mermaid = { initialize() {}, render: async () => {
  globalThis.__katanaMermaidDiagramType = 'flowchart';
  const original = globalThis.__katanaMeasureCanvasText;
  const calls = [];
  globalThis.__katanaMeasureCanvasText = (...args) => { calls.push(args); return original(...args); };
  const canvas = document.createElement('canvas').getContext('2d');
  canvas.font = '16px Hiragino Sans, sans-serif';
  const width = canvas.measureText('データベース').width;
  const call = calls[0];
  return { svg: '<svg data-segments="' + call[5].join('|') + '" data-width="' + (width > 0) + '"></svg>' };
}};
"##;

const CANVAS_BOUNDARY_FIXTURE: &str = r##"
globalThis.mermaid = { initialize() {}, render: async () => {
  globalThis.__katanaMermaidDiagramType = 'flowchart';
  const original = globalThis.__katanaMeasureCanvasText;
  const calls = [];
  globalThis.__katanaMeasureCanvasText = (...args) => {
    calls.push(args);
    return args[0] === 'AV' ? 1.25 : original(...args);
  };
  const canvas = document.createElement('canvas').getContext('2d');
  canvas.font = '16px Hiragino Sans, sans-serif';
  const ascii = canvas.measureText('AV').width;
  canvas.measureText('ース');
  canvas.measureText('日本,abc');
  canvas.measureText('résumé.test');
  canvas.measureText('が');
  canvas.measureText('👩‍💻');
  const segments = calls.slice(1).map((call) => call[5].join('|'));
  return { svg: '<svg data-ascii="' + (ascii === 1.25) + '" data-leading="' + segments[0] +
    '" data-punctuation="' + segments[1] + '" data-latin="' + segments[2] +
    '" data-mark="' + segments[3] +
    '" data-emoji="' + segments[4] + '"></svg>' };
}};
"##;

const CANVAS_SEGMENTED_EXCEPTION_FIXTURE: &str = r##"
globalThis.mermaid = { initialize() {}, render: async () => {
  globalThis.__katanaMermaidDiagramType = 'architecture';
  globalThis.__katanaMeasureCanvasText = () => { throw new Error('segmented bridge failure'); };
  const canvas = document.createElement('canvas').getContext('2d');
  canvas.font = '16px Hiragino Sans, sans-serif';
  canvas.measureText('データ');
  return { svg: '<svg></svg>' };
}};
"##;

const CANVAS_SEGMENTED_INVALID_FIXTURE: &str = r##"
globalThis.mermaid = { initialize() {}, render: async () => {
  globalThis.__katanaMermaidDiagramType = 'architecture';
  globalThis.__katanaMeasureCanvasText = () => undefined;
  const canvas = document.createElement('canvas').getContext('2d');
  canvas.font = '16px Hiragino Sans, sans-serif';
  canvas.measureText('データ');
  return { svg: '<svg></svg>' };
}};
"##;

const CANVAS_EXCEPTION_FIXTURE: &str = r##"
globalThis.mermaid = { initialize() {}, render: async () => {
  globalThis.__katanaMermaidDiagramType = 'architecture';
  globalThis.__katanaMeasureCanvasText = undefined;
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
  globalThis.__katanaMeasureCanvasText = undefined;
  globalThis.__katanaMeasureText = () => undefined;
  const canvas = document.createElement('canvas').getContext('2d');
  canvas.font = '16px Helvetica, sans-serif';
  canvas.measureText('AV');
  return { svg: '<svg></svg>' };
}};
"##;
