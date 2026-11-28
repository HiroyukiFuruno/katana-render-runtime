use super::raw_metrics::svg_fixture_with_independent_raw_advances;
use super::render_svg_fixture;

const SAME_FONT_SHAPED_RUN_FIXTURE: &str = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    globalThis.__katanaMermaidDiagramType = 'classdiagram';
    const ns = 'http://www.w3.org/2000/svg';
    const family = 'trebuchet ms, verdana, arial, sans-serif';
    const expected = globalThis.__katanaMeasureText('AVA', family, 16, 400, false);
    const widths = ['nested', 'mixed', 'direct'].map((mode) => {
      const text = document.createElementNS(ns, 'text');
      const outer = mode === 'direct' ? text : document.createElementNS(ns, 'tspan');
      if (mode !== 'direct') text.appendChild(outer);
      const middle = document.createElementNS(ns, 'tspan');
      middle.textContent = 'V';
      if (mode === 'nested') {
        const first = document.createElementNS(ns, 'tspan');
        const last = document.createElementNS(ns, 'tspan');
        first.textContent = 'A';
        last.textContent = 'A';
        outer.appendChild(first);
        outer.appendChild(middle);
        outer.appendChild(last);
      } else {
        outer.appendChild(document.createTextNode('A'));
        outer.appendChild(middle);
        outer.appendChild(document.createTextNode('A'));
      }
      return text.getBBox().width;
    });
    return { svg: '<svg data-shaped-runs="' + widths.every((width) => Math.abs(width - expected) < 0.0001) + '"></svg>' };
  }
};
"##;

const SAME_FONT_WORD_BOUNDARY_FIXTURE: &str = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    globalThis.__katanaMermaidDiagramType = 'classdiagram';
    const ns = 'http://www.w3.org/2000/svg';
    const family = 'trebuchet ms, verdana, arial, sans-serif';
    const cases = [
      { owners: ['-int', ' sizeInFeet'], groups: ['-int', ' sizeInFeet'] },
      { owners: [' AV', 'A'], groups: [' AVA'] },
      { owners: ['AV', 'A tail '], groups: ['AVA tail '] },
      { owners: ['A', 'V', 'A tail ', ' B'], groups: ['AVA tail ', ' B'] },
    ];
    const expectedRuns = __KATANA_RUST_RAW_SVG_ADVANCES__;
    let expectedIndex = 0;
    const observed = cases.map(({ owners, groups }) => {
      const text = document.createElementNS(ns, 'text');
      const outer = document.createElementNS(ns, 'tspan');
      owners.forEach(value => {
        const owner = document.createElementNS(ns, 'tspan');
        owner.textContent = value;
        outer.appendChild(owner);
      });
      text.appendChild(outer);
      const expectedWidth = groups.reduce((sum, value) => {
        const advance = expectedRuns[expectedIndex++];
        return sum + Math.ceil(advance * 64) / 64;
      }, 0);
      const rootBox = text.getBBox();
      return {
        expectedWidth,
        actualWidth: rootBox.width,
        actualLength: text.getComputedTextLength(),
        mergedWidth: globalThis.__katanaMeasureSvgTextMetrics(owners.join(''), family, 16, 400, false).advance,
      };
    });
    const correct = observed.every(({ expectedWidth, actualWidth, actualLength }) =>
      Math.abs(actualWidth - expectedWidth) < 0.0001
        && Math.abs(actualLength - expectedWidth) < 0.0001,
    );
    return { svg: '<svg data-word-boundary="' + (correct && expectedIndex === expectedRuns.length)
      + '" data-word-boundary-details="' + JSON.stringify({ family, expectedRuns, observed })
        .replaceAll('&', '&amp;').replaceAll('"', '&quot;') + '"></svg>' };
  }
};
"##;

const NESTED_TSPAN_MEASURE_FIXTURE: &str = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    globalThis.__katanaMermaidDiagramType = 'classdiagram';
    const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
    const outer = document.createElementNS('http://www.w3.org/2000/svg', 'tspan');
    const first = document.createElementNS('http://www.w3.org/2000/svg', 'tspan');
    const second = document.createElementNS('http://www.w3.org/2000/svg', 'tspan');
    first.textContent = '-int';
    second.textContent = ' sizeInFeet';
    second.style.setProperty('font-weight', 'bold');
    outer.appendChild(first);
    outer.appendChild(second);
    text.appendChild(outer);
    const family = 'trebuchet ms, verdana, arial, sans-serif';
    const expectedRuns = __KATANA_RUST_RAW_SVG_ADVANCES__;
    const layoutExpectedRuns = expectedRuns.map(width => Math.ceil(width * 64) / 64);
    const cssWidths = [['-int', 400], [' sizeInFeet', 700]]
      .map(([value, weight]) => globalThis.__katanaMeasureText(value, family, 16, weight, false));
    const cssContract = cssWidths.every((width, index) => width === Math.ceil(expectedRuns[index] * 64) / 64);
    const parts = katanaOrderedTextLines(text).flatMap(line => line.parts);
    const orderedRuns = parts.length === 2 && parts[0].text === '-int' && parts[1].text === ' sizeInFeet'
      && parts[0].font.weight === 400 && parts[1].font.weight === 700
      && parts.every((part, index) => part.font.family === family && part.font.size === 16 && !part.font.italic
        && Math.abs(part.width - expectedRuns[index]) < 0.0001);
    const rootBox = text.getBBox();
    const leafBoxes = [first.getBBox(), second.getBBox()];
    const rootTextLength = text.getComputedTextLength();
    const leafTextLengths = [first.getComputedTextLength(), second.getComputedTextLength()];
    const layoutTotal = layoutExpectedRuns.reduce((sum, width) => sum + width, 0);
    const bboxContract = Math.abs(rootBox.width - layoutTotal) < 0.0001
      && leafBoxes.every((box, index) => Math.abs(box.width - layoutExpectedRuns[index]) < 0.0001)
      && Math.abs(rootTextLength - layoutTotal) < 0.0001
      && leafTextLengths.every((length, index) => Math.abs(length - layoutExpectedRuns[index]) < 0.0001);
    const details = JSON.stringify({ expectedRuns, layoutExpectedRuns, cssWidths, rootBox, leafBoxes,
      rootTextLength, leafTextLengths, orderedRuns, cssContract, bboxContract })
      .replaceAll('&', '&amp;').replaceAll('"', '&quot;');
    return { svg: '<svg data-nested-runs="' + (bboxContract && orderedRuns && cssContract)
      + '" data-nested-details="' + details + '"></svg>' };
  }
};
"##;

const NESTED_TEXT_RUNS_FIXTURE_PREFIX: &str = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    globalThis.__katanaMermaidDiagramType = 'classdiagram';
    const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
"##;

const MIXED_NESTED_TSPAN_NODES: &str = r##"    const outer = document.createElementNS('http://www.w3.org/2000/svg', 'tspan');
    const nested = document.createElementNS('http://www.w3.org/2000/svg', 'tspan');
    nested.textContent = 'B';
    nested.style.setProperty('font-weight', 'bold');
    outer.appendChild(document.createTextNode('A'));
    outer.appendChild(nested);
    outer.appendChild(document.createTextNode('C'));
    text.appendChild(outer);
"##;

const DIRECT_TEXT_NESTED_TSPAN_NODES: &str = r##"    const nested = document.createElementNS('http://www.w3.org/2000/svg', 'tspan');
    nested.textContent = 'B';
    nested.style.setProperty('font-weight', 'bold');
    text.appendChild(document.createTextNode('A'));
    text.appendChild(nested);
    text.appendChild(document.createTextNode('C'));
"##;

const NESTED_TEXT_RUNS_MEASURE_CHECKS: &str = r##"    const family = 'trebuchet ms, verdana, arial, sans-serif';
    const expectedRuns = __KATANA_RUST_RAW_SVG_ADVANCES__;
    const layoutExpectedRuns = expectedRuns.map(width => Math.ceil(width * 64) / 64);
    const cssWidths = [['A', 400], ['B', 700], ['C', 400]]
      .map(([value, weight]) => globalThis.__katanaMeasureText(value, family, 16, weight, false));
    const cssContract = cssWidths.every((width, index) => width === Math.ceil(expectedRuns[index] * 64) / 64);
    const parts = katanaOrderedTextLines(text).flatMap(line => line.parts);
    const orderedRuns = parts.length === 3 && parts.map(part => part.text).join('') === 'ABC'
      && parts[0].font.weight === 400 && parts[1].font.weight === 700 && parts[2].font.weight === 400
      && parts.every((part, index) => part.text === ['A', 'B', 'C'][index]
        && part.font.family === family && part.font.size === 16 && !part.font.italic
        && Math.abs(part.width - expectedRuns[index]) < 0.0001);
    const rootBox = text.getBBox();
    const nestedBox = nested.getBBox();
    const rootTextLength = text.getComputedTextLength();
    const nestedTextLength = nested.getComputedTextLength();
    const expected = layoutExpectedRuns.reduce((sum, width) => sum + width, 0);
    const bboxContract = Math.abs(rootBox.width - expected) < 0.0001
      && Math.abs(nestedBox.width - layoutExpectedRuns[1]) < 0.0001
      && Math.abs(rootTextLength - expected) < 0.0001
      && Math.abs(nestedTextLength - layoutExpectedRuns[1]) < 0.0001;
    const details = JSON.stringify({ expectedRuns, layoutExpectedRuns, cssWidths, rootBox, nestedBox,
      rootTextLength, nestedTextLength, orderedRuns, cssContract, bboxContract })
      .replaceAll('&', '&amp;').replaceAll('"', '&quot;');
    return { svg: '<svg data-"##;

const NESTED_TEXT_RUNS_RESULT_SUFFIX: &str = r##"="' + (bboxContract && orderedRuns && cssContract)
  + '" data-details="' + details + '"></svg>' };
  }
};
"##;

const NESTED_TEXT_RUNS_FIXTURE_INITIAL_CAPACITY_BYTES: usize = 2048;

fn nested_text_runs_measure_fixture(nodes: &str, marker: &str) -> String {
    let mut fixture = String::with_capacity(NESTED_TEXT_RUNS_FIXTURE_INITIAL_CAPACITY_BYTES);
    fixture.push_str(NESTED_TEXT_RUNS_FIXTURE_PREFIX);
    fixture.push_str(nodes);
    fixture.push_str(NESTED_TEXT_RUNS_MEASURE_CHECKS);
    fixture.push_str(marker);
    fixture.push_str(NESTED_TEXT_RUNS_RESULT_SUFFIX);
    fixture
}

fn mixed_nested_tspan_measure_fixture_mermaid() -> String {
    nested_text_runs_measure_fixture(MIXED_NESTED_TSPAN_NODES, "mixed-runs")
}

#[test]
fn svg_nested_tspan_bbox_sums_browser_font_runs() -> Result<(), String> {
    let fixture = svg_fixture_with_independent_raw_advances(
        NESTED_TSPAN_MEASURE_FIXTURE,
        &[("-int", 400), (" sizeInFeet", 700)],
    )?;
    let rendered = render_svg_fixture(&fixture)?;
    assert!(
        rendered.contains(r##"data-nested-runs="true"##),
        "{rendered}"
    );
    Ok(())
}

#[test]
fn svg_mixed_nested_tspan_bbox_keeps_text_node_runs() -> Result<(), String> {
    let base_fixture = mixed_nested_tspan_measure_fixture_mermaid();
    let fixture = svg_fixture_with_independent_raw_advances(
        &base_fixture,
        &[("A", 400), ("B", 700), ("C", 400)],
    )?;
    let rendered = render_svg_fixture(&fixture)?;
    assert!(
        rendered.contains(r##"data-mixed-runs="true"##),
        "{rendered}"
    );
    Ok(())
}

#[test]
fn svg_direct_text_with_nested_tspan_keeps_text_node_runs() -> Result<(), String> {
    let base_fixture = direct_text_nested_tspan_measure_fixture_mermaid();
    let fixture = svg_fixture_with_independent_raw_advances(
        &base_fixture,
        &[("A", 400), ("B", 700), ("C", 400)],
    )?;
    let rendered = render_svg_fixture(&fixture)?;
    assert!(
        rendered.contains(r##"data-direct-runs="true"##),
        "{rendered}"
    );
    Ok(())
}

#[test]
fn svg_same_font_nested_text_preserves_shaped_run_kerning() -> Result<(), String> {
    let rendered = render_svg_fixture(SAME_FONT_SHAPED_RUN_FIXTURE)?;
    assert!(
        rendered.contains(r##"data-shaped-runs="true"##),
        "{rendered}"
    );
    Ok(())
}

#[test]
fn svg_same_font_word_boundary_splits_only_actual_owner_boundaries() -> Result<(), String> {
    let fixture = svg_fixture_with_independent_raw_advances(
        SAME_FONT_WORD_BOUNDARY_FIXTURE,
        &[
            ("-int", 400),
            (" sizeInFeet", 400),
            (" AVA", 400),
            ("AVA tail ", 400),
            ("AVA tail ", 400),
            (" B", 400),
        ],
    )?;
    let rendered = render_svg_fixture(&fixture)?;
    assert!(
        rendered.contains(r##"data-word-boundary="true"##),
        "{rendered}"
    );
    Ok(())
}

fn direct_text_nested_tspan_measure_fixture_mermaid() -> String {
    nested_text_runs_measure_fixture(DIRECT_TEXT_NESTED_TSPAN_NODES, "direct-runs")
}
