use super::MermaidRuntimeScripts;
use crate::markdown::diagram_js_runtime::DiagramV8Runtime;

#[path = "js_runtime_scripts_svg_bbox_float_tests.rs"]
mod bbox_float;
#[path = "js_runtime_scripts_canvas_context_tests.rs"]
mod canvas_context;
#[path = "js_runtime_scripts_svg_computed_style_tests.rs"]
mod computed_style;
#[path = "js_runtime_scripts_svg_namespace_tests.rs"]
mod namespace;
#[path = "js_runtime_scripts_svg_style_tests.rs"]
mod style;
#[path = "js_runtime_scripts_svg_text_runs_tests.rs"]
mod text_runs;

fn render_svg_fixture(mermaid: &str) -> Result<String, String> {
    let request = serde_json::json!({
        "source": "classDiagram",
        "svgId": "id",
        "theme": "dark",
        "background": "#000",
        "fill": "#111",
        "text": "#fff",
        "stroke": "#fff",
        "arrow": "#fff",
    })
    .to_string();
    let scripts = MermaidRuntimeScripts::build_with_zenuml(mermaid, "", &request);
    DiagramV8Runtime::render(&scripts)
}

#[test]
fn svg_group_bbox_preserves_negative_child_text_origin() -> Result<(), String> {
    let mermaid = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    const parent = document.createElementNS('http://www.w3.org/2000/svg', 'g');
    const child = document.createElementNS('http://www.w3.org/2000/svg', 'g');
    const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
    text.setAttribute('text-anchor', 'middle');
    text.setAttribute('x', '0');
    text.textContent = 'RenderedSection';
    child.appendChild(text);
    parent.appendChild(child);
    const box = parent.getBBox();
    return { svg: '<svg data-group-box="' + (box.x < 0) + ',' + (box.width > 100) + '"></svg>' };
  }
};
"##;
    let rendered = render_svg_fixture(mermaid)?;
    assert!(
        rendered.contains(r##"data-group-box="true,true"##),
        "{rendered}"
    );
    Ok(())
}

#[test]
fn svg_nested_group_bbox_does_not_repeat_parent_text() -> Result<(), String> {
    let mermaid = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    const parent = document.createElementNS('http://www.w3.org/2000/svg', 'g');
    const child = document.createElementNS('http://www.w3.org/2000/svg', 'text');
    parent.textContent = 'RenderedSection';
    child.textContent = 'A';
    parent.appendChild(child);
    const box = parent.getBBox();
    return { svg: '<svg data-group-box="' + box.width.toFixed(3) + '"></svg>' };
  }
};
"##;
    let rendered = render_svg_fixture(mermaid)?;
    assert!(
        rendered.contains(r##"data-group-box="9.438"##),
        "{rendered}"
    );
    Ok(())
}

#[test]
fn svg_class_title_bbox_applies_bolder_font_weight() -> Result<(), String> {
    let mermaid = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    const normal = document.createElementNS('http://www.w3.org/2000/svg', 'text');
    normal.textContent = 'RenderedSection';
    const bold = document.createElementNS('http://www.w3.org/2000/svg', 'text');
    bold.style.setProperty('font-weight', 'bolder');
    bold.textContent = 'RenderedSection';
    return { svg: '<svg data-bolder="' + (bold.getBBox().width > normal.getBBox().width) + '"></svg>' };
  }
};
"##;
    let rendered = render_svg_fixture(mermaid)?;
    assert!(rendered.contains(r##"data-bolder="true"##), "{rendered}");
    Ok(())
}

#[test]
fn svg_class_font_measure_bridge_returns_numeric_advance() -> Result<(), String> {
    let rendered = render_svg_fixture(font_measure_fixture_mermaid())?;
    assert!(
        rendered.contains(r##"data-font-measure-type="number" data-font-measure-finite="true" data-font-measure-invalid="undefined""##),
        "{rendered}"
    );
    Ok(())
}

#[test]
fn font_measure_bridge_rejects_non_numeric_size_and_uses_defaults() -> Result<(), String> {
    let source = r##"globalThis.mermaid = {
      initialize() {},
      render: async () => {
        const measure = globalThis.__katanaMeasureText;
        const fallback = measure('AV', 'sans-serif');
        let rejected = false;
        try { measure('AV', 'sans-serif', Symbol()); } catch (error) { rejected = error instanceof TypeError; }
        return {svg: '<svg data-default="' + Number.isFinite(fallback) + '" data-invalid="' + rejected + '"></svg>'};
      }
    };"##;
    let rendered = render_svg_fixture(source)?;
    assert!(
        rendered.contains(r##"data-default="true" data-invalid="true""##),
        "{rendered}"
    );
    Ok(())
}

#[test]
fn font_measure_bridge_rejects_css_size_overflow() -> Result<(), String> {
    let source = r##"globalThis.mermaid = {
      initialize() {},
      render: async () => {
        const width = globalThis.__katanaMeasureText('AV', 'sans-serif', Number.MAX_VALUE);
        return {svg: '<svg data-overflow="' + (width === undefined) + '"></svg>'};
      }
    };"##;
    let rendered = render_svg_fixture(source)?;
    assert!(rendered.contains(r##"data-overflow="true""##), "{rendered}");
    Ok(())
}

fn font_measure_fixture_mermaid() -> &'static str {
    r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    const width = globalThis.__katanaMeasureText(
      'RenderedSection',
      'trebuchet ms, verdana, arial, sans-serif',
      16,
      700,
      false,
    );
    const invalid = globalThis.__katanaMeasureText(
      'RenderedSection',
      'trebuchet ms, verdana, arial, sans-serif',
      NaN,
      700,
      false,
    );
    return { svg: '<svg data-font-measure-type="' + typeof width + '" data-font-measure-finite="' + Number.isFinite(width) + '" data-font-measure-invalid="' + typeof invalid + '"></svg>' };
  }
};
"##
}

#[test]
fn svg_group_bbox_parses_single_and_scientific_translate_arguments() -> Result<(), String> {
    let rendered = render_svg_fixture(translate_fixture_mermaid())?;
    assert!(
        rendered.contains(r##"data-translate-box="-53.5395"##),
        "{rendered}"
    );
    assert!(rendered.contains(r##"data-invalid-box="0"##), "{rendered}");
    Ok(())
}

fn translate_fixture_mermaid() -> &'static str {
    r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    const parent = document.createElementNS('http://www.w3.org/2000/svg', 'g');
    for (const transform of ['translate(-53.5395)', 'translate(1e1,2e1)', 'translate(3 4)']) {
      const group = document.createElementNS('http://www.w3.org/2000/svg', 'g');
      const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
      group.setAttribute('transform', transform);
      text.textContent = 'A';
      group.appendChild(text);
      parent.appendChild(group);
    }
    const box = parent.getBBox();
    const invalidParent = document.createElementNS('http://www.w3.org/2000/svg', 'g');
    const invalid = document.createElementNS('http://www.w3.org/2000/svg', 'g');
    const invalidText = document.createElementNS('http://www.w3.org/2000/svg', 'text');
    invalid.setAttribute('transform', 'translate(1e--2,3)');
    invalidText.textContent = 'A';
    invalid.appendChild(invalidText);
    invalidParent.appendChild(invalid);
    const invalidBox = invalidParent.getBBox();
    return { svg: '<svg data-translate-box="' + box.x + '" data-invalid-box="' + invalidBox.x + '"></svg>' };
  }
};
"##
}
