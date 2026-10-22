use super::render_svg_fixture;

const SVG_LENGTH_FLOAT32_POSITION_FIXTURE: &str = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    globalThis.__katanaMermaidDiagramType = 'classdiagram';
    const ns = 'http://www.w3.org/2000/svg';
    const cases = [
      { first: { y: '-0.1em' }, second: { dy: '1.1em' }, expected: [{ y: 14.000000953674316 }] },
      { first: { x: '-2px' }, second: { dx: '3px' }, expected: [{ x: 1 }] },
      { first: { y: '-1px' }, second: { dy: '2px' }, expected: [{ y: 1 }] },
      { first: { y: '-1px' }, second: { dy: '2px' }, third: { dy: '3px' }, expected: [{ y: 4 }] },
    ];
    const observed = cases.map(({ first, second, third, expected }) => {
      const text = document.createElementNS(ns, 'text');
      text.style.setProperty('font-family', 'trebuchet ms, verdana, arial, sans-serif');
      text.style.setProperty('font-size', '14px');
      let parent = text;
      [first, second, third].filter(Boolean).forEach((attrs) => {
        const tspan = document.createElementNS(ns, 'tspan');
        Object.entries(attrs).forEach(([name, value]) => tspan.setAttribute(name, value));
        parent.appendChild(tspan);
        parent = tspan;
      });
      parent.textContent = 'leaf';
      const positions = katanaOrderedTextLines(text).flatMap(line => line.parts)
        .map(part => ({ x: part.x, y: part.y }));
      const matches = positions.length === expected.length
        && positions.every((position, index) => Object.entries(expected[index])
          .every(([axis, value]) => Math.abs(position[axis] - value) < 0.000001));
      return { matches, positions, expected };
    });
    const passed = observed.every(({ matches }) => matches);
    const details = JSON.stringify(observed).replaceAll('&', '&amp;').replaceAll('"', '&quot;');
    return { svg: '<svg data-svg-length-float32="' + passed + '" data-svg-length-float32-details="'
      + details + '"></svg>' };
  }
};
"##;

const SVG_LENGTH_POSITION_OVERFLOW_FIXTURE: &str = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    const ns = 'http://www.w3.org/2000/svg';
    const text = document.createElementNS(ns, 'text');
    const first = document.createElementNS(ns, 'tspan');
    first.setAttribute('x', '3.4e38px');
    first.textContent = 'first';
    const second = document.createElementNS(ns, 'tspan');
    second.setAttribute('dx', '3.4e38px');
    second.textContent = 'second';
    text.append(first, second);
    katanaOrderedTextLines(text);
    return { svg: '<svg></svg>' };
  }
};
"##;

#[test]
fn svg_length_positions_match_chrome_float32_boundaries() -> Result<(), String> {
    let rendered = render_svg_fixture(SVG_LENGTH_FLOAT32_POSITION_FIXTURE)?;
    assert!(
        rendered.contains(r##"data-svg-length-float32="true"##),
        "{rendered}"
    );
    Ok(())
}

#[test]
fn svg_length_position_overflow_is_rejected() -> Result<(), String> {
    let error = match render_svg_fixture(SVG_LENGTH_POSITION_OVERFLOW_FIXTURE) {
        Ok(svg) => {
            return Err(format!(
                "SVG position overflow unexpectedly rendered: {svg}"
            ));
        }
        Err(error) => error,
    };
    if !error.to_ascii_lowercase().contains("overflow") {
        return Err(format!(
            "SVG position error did not identify overflow: {error}"
        ));
    }
    Ok(())
}
