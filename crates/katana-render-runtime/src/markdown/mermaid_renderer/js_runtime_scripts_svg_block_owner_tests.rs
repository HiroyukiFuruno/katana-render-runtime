use super::super::render_svg_fixture;
use super::MermaidRuntimeScripts;
use crate::markdown::diagram_js_runtime::DiagramV8Runtime;

const BLOCK_SAME_FONT_OWNER_FIXTURE: &str = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    globalThis.__katanaMermaidDiagramType = 'block';
    const ns = 'http://www.w3.org/2000/svg';
    const family = 'trebuchet ms, verdana, arial, sans-serif';
    const expected = globalThis.__katanaMeasureSvgTextMetrics('AVA', family, 16, 400, false).advance;
    const expectedVa = globalThis.__katanaMeasureSvgTextMetrics('VA', family, 16, 400, false).advance;
    const make = (attrs = {}, style = {}) => {
      const text = document.createElementNS(ns, 'text');
      text.style.setProperty('font-family', family);
      text.style.setProperty('font-size', '16px');
      for (const [name, value] of Object.entries(attrs)) text.setAttribute(name, value);
      for (const [name, value] of Object.entries(style)) text.style.setProperty(name, value);
      for (const part of ['A', 'V', 'A']) {
        const tspan = document.createElementNS(ns, 'tspan');
        tspan.textContent = part;
        text.appendChild(tspan);
      }
      return text;
    };
    const ltr = make();
    const ltrParts = katanaOrderedTextLines(ltr).flatMap(line => line.parts);
    const positioned = make();
    positioned.children[1].setAttribute('x', '40');
    const positionedLines = katanaOrderedTextLines(positioned);
    const positionedParts = positionedLines.flatMap(line => line.parts);
    const styled = make();
    styled.children[1].style.setProperty('font-weight', '700');
    const styledParts = katanaOrderedTextLines(styled).flatMap(line => line.parts);
    const rtl = make({ direction: 'rtl' });
    const rtlParts = katanaOrderedTextLines(rtl).flatMap(line => line.parts);
    const bidi = make({}, { 'unicode-bidi': 'bidi-override' });
    const bidiParts = katanaOrderedTextLines(bidi).flatMap(line => line.parts);
    const newline = make();
    newline.children[1].setAttribute('dy', '1em');
    const newlineLines = katanaOrderedTextLines(newline);
    const safe = read => { try { return read(); } catch (error) { return String(error); } };
    const describe = (node, lines) => ({
      partCount: lines.flatMap(line => line.parts).length,
      lineCount: lines.length,
      linePartCounts: lines.map(line => line.parts.length),
      partWidths: lines.flatMap(line => line.parts.map(part => part.width)),
      owners: lines.flatMap(line => line.parts).map(part => ({
        localName: part.node?.localName,
        className: part.node?.getAttribute?.('class'),
        text: part.text,
        canMerge: safe(() => katanaCanMergeNativeTextOwners(part.node)),
        resolvedFont: safe(() => katanaResolvedNodeTextFont(part.node)),
        measuredFont: part.font,
      })),
      direction: {
        attribute: node.getAttribute('direction'),
        inline: node.style.getPropertyValue('direction'),
        computed: safe(() => katanaCssComputedStyleValue(node, 'direction')),
      },
      unicodeBidi: {
        attribute: node.getAttribute('unicode-bidi'),
        inline: node.style.getPropertyValue('unicode-bidi'),
        computed: safe(() => katanaCssComputedStyleValue(node, 'unicode-bidi')),
      },
      bboxWidth: safe(() => node.getBBox().width),
      computedTextLength: safe(() => node.getComputedTextLength()),
    });
    const inspectPending = node => safe(() => {
      const context = katanaTextFontContext(node);
      const presence = new WeakMap();
      katanaTextRunPresence(node, presence);
      const state = { lines: [], current: null, pending: null, x: 0, y: 0, css: context.css, presence };
      katanaVisitTextRuns(node, state, context.inherited);
      const pending = state.pending;
      return {
        pendingText: pending?.texts.join(''),
        pendingSvgNativeEligible: pending?.svgNativeEligible,
        pendingFont: pending?.font,
        pendingCanMerge: pending ? katanaCanMergeNativeTextOwners(pending) : null,
        fragments: pending?.fragments.map(fragment => ({
          localName: fragment.node?.localName,
          className: fragment.node?.getAttribute?.('class'),
          text: fragment.texts.join(''),
          ownerCanMerge: katanaCanMergeNativeTextOwners(fragment.node),
          resolvedFont: katanaResolvedNodeTextFont(fragment.node),
        })),
      };
    });
    const diagnostics = {
      diagramType: globalThis.__katanaMermaidDiagramType,
      nativeAdvanceRuntime: safe(() => katanaMermaidNativeAdvanceRuntime()),
      measureTextType: typeof globalThis.__katanaMeasureText,
      svgMetricsType: typeof globalThis.__katanaMeasureSvgTextMetrics,
      expectedAdvance: expected,
      ltrPendingBeforeFlush: inspectPending(ltr),
      ltr: describe(ltr, katanaOrderedTextLines(ltr)),
      positioned: describe(positioned, katanaOrderedTextLines(positioned)),
      styled: describe(styled, katanaOrderedTextLines(styled)),
      rtl: describe(rtl, katanaOrderedTextLines(rtl)),
      bidi: describe(bidi, katanaOrderedTextLines(bidi)),
      newline: describe(newline, newlineLines),
      newlineExpectedLineCount: 2,
    };
    const result = ltrParts.length === 1 && Math.abs(ltrParts[0].width - expected) < 0.0001
      && Math.abs(ltr.getBBox().width - expected) < 0.0001
      && positionedLines.length === 2 && positionedParts.length === 2
      && positionedLines[0].text === 'A' && positionedLines[1].text === 'VA'
      && positionedLines[1].x === 40
      && Math.abs(positioned.getBBox().width - (40 + expectedVa)) < 0.0001
      && styledParts.length === 3 && rtlParts.length === 3 && bidiParts.length === 3
      && newlineLines.length === 2;
    return {
      svg: '<svg data-block-run-merge-guards="' + result + '" data-block-run-diagnostic="'
        + encodeURIComponent(JSON.stringify(diagnostics)) + '"></svg>',
    };
  }
};
"##;

const BLOCK_BETA_RAW_REQUEST_FIXTURE: &str = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    const ns = 'http://www.w3.org/2000/svg';
    const family = 'trebuchet ms, verdana, arial, sans-serif';
    const expected = globalThis.__katanaMeasureSvgTextMetrics('AVA', family, 16, 400, false).advance;
    const text = document.createElementNS(ns, 'text');
    text.style.setProperty('font-family', family);
    text.style.setProperty('font-size', '16px');
    for (const part of ['A', 'V', 'A']) {
      const tspan = document.createElementNS(ns, 'tspan');
      tspan.textContent = part;
      text.appendChild(tspan);
    }
    const lines = katanaOrderedTextLines(text);
    const parts = lines.flatMap(line => line.parts);
    const result = globalThis.__katanaMermaidDiagramType === 'block-beta'
      && katanaIsBlockDiagram()
      && parts.length === 1 && parts[0].text === 'AVA'
      && Math.abs(parts[0].width - expected) < 0.0001
      && Math.abs(text.getBBox().width - expected) < 0.0001;
    return { svg: '<svg data-block-beta-raw-route="' + result + '" data-diagram-type="'
      + globalThis.__katanaMermaidDiagramType + '"></svg>' };
  }
};
"##;

const C4_SAME_FONT_OWNER_FIXTURE: &str = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    globalThis.__katanaMermaidDiagramType = 'c4dynamic';
    const ns = 'http://www.w3.org/2000/svg';
    const family = 'trebuchet ms, verdana, arial, sans-serif';
    const textValue = 'Banking Customer A';
    const make = (attrs = {}, style = {}) => {
      const text = document.createElementNS(ns, 'text');
      text.style.setProperty('font-family', family);
      text.style.setProperty('font-size', '16px');
      for (const [name, value] of Object.entries(attrs)) text.setAttribute(name, value);
      for (const [name, value] of Object.entries(style)) text.style.setProperty(name, value);
      for (const part of ['Banking', ' Customer', ' A']) {
        const tspan = document.createElementNS(ns, 'tspan');
        tspan.setAttribute('class', 'c4-name');
        tspan.textContent = part;
        text.appendChild(tspan);
      }
      return text;
    };
    const ltr = make();
    const ltrLines = katanaOrderedTextLines(ltr);
    const positioned = make();
    positioned.children[1].setAttribute('x', '40');
    const positionedLines = katanaOrderedTextLines(positioned);
    const rtl = make({ direction: 'rtl' });
    const rtlLines = katanaOrderedTextLines(rtl);
    const bidi = make({}, { 'unicode-bidi': 'bidi-override' });
    const bidiLines = katanaOrderedTextLines(bidi);
    const styled = make();
    styled.children[1].style.setProperty('font-weight', '700');
    const styledLines = katanaOrderedTextLines(styled);
    const result = ltrLines.length === 1 && ltrLines[0].parts.length === 3
      && ltrLines[0].parts.map(part => part.text).join('') === textValue
      && ltrLines[0].parts.every(part => part.node?.getAttribute('class') === 'c4-name')
      && positionedLines.length === 2
      && rtlLines.flatMap(line => line.parts).length === 3
      && bidiLines.flatMap(line => line.parts).length === 3
      && styledLines.flatMap(line => line.parts).length === 3;
    return { svg: '<svg data-c4-run-merge-guards="' + result + '"></svg>' };
  }
};
"##;

#[test]
fn block_same_font_owners_shape_as_one_run_only_for_ltr_normal_bidi() -> Result<(), String> {
    let rendered = render_svg_fixture(BLOCK_SAME_FONT_OWNER_FIXTURE)?;
    assert!(
        rendered.contains(r##"data-block-run-merge-guards="true"##),
        "{rendered}"
    );
    Ok(())
}

#[test]
fn block_beta_raw_request_uses_native_owner_merge_guard() -> Result<(), String> {
    let rendered =
        render_svg_fixture_with_source(BLOCK_BETA_RAW_REQUEST_FIXTURE, "block-beta\nA[AVA]")?;
    assert!(
        rendered.contains(r##"data-block-beta-raw-route="true""##),
        "{rendered}"
    );
    Ok(())
}

#[test]
fn c4_same_font_owners_keep_owner_boundaries_even_with_same_font() -> Result<(), String> {
    let rendered = render_svg_fixture(C4_SAME_FONT_OWNER_FIXTURE)?;
    assert!(
        rendered.contains(r##"data-c4-run-merge-guards="true"##),
        "{rendered}"
    );
    Ok(())
}

fn render_svg_fixture_with_source(mermaid: &str, source: &str) -> Result<String, String> {
    let request = serde_json::json!({
        "source": source,
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
