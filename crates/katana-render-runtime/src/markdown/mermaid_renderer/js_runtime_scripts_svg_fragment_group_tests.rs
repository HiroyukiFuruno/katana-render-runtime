use super::render_svg_fixture;

type FragmentObservation<'a> = (&'a str, bool, f64);

const C4_FRAGMENT_OBSERVATION_FIXTURE: &str = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    const ns = 'http://www.w3.org/2000/svg';
    const original = globalThis.__katanaMeasureSvgFragmentLayout;
    let calls = 0;
    let currentMode = '';
    globalThis.__katanaMeasureSvgFragmentLayout = (...args) => {
      calls += 1;
      return currentMode === 'unsupported' ? null : original(...args);
    };
    const observed = [];
    for (const mode of ['plain', 'translation', 'unsupported', 'non-c4', 'style', 'font',
      'position', 'direction', 'word-spacing', 'mixed-text', 'mixed-space', 'textLength',
      'lengthAdjust', 'rotate', 'multi-x', 'multi-y', 'multi-dy', 'row-dx', 'ancestor-dx',
      'stretch', 'caps', 'features', 'variations', 'ligatures', 'scale', 'rotation', 'skew',
      'css-transform', 'viewBox']) {
      currentMode = mode;
      globalThis.__katanaMermaidDiagramType = mode === 'non-c4' ? 'flowchart' : 'c4context';
      const svg = document.createElementNS(ns, 'svg');
      const group = document.createElementNS(ns, 'g');
      const text = document.createElementNS(ns, 'text');
      const row = document.createElementNS(ns, 'tspan');
      row.setAttribute('x', '0'); row.setAttribute('text-anchor', 'middle');
      if (mode === 'word-spacing') text.style.setProperty('word-spacing', '1px');
      if (mode === 'translation') group.setAttribute('transform', 'translate(12, 9)');
      if (mode === 'scale') group.setAttribute('transform', 'scale(0.99999434)');
      if (mode === 'rotation') group.setAttribute('transform', 'rotate(10)');
      if (mode === 'skew') group.setAttribute('transform', 'skewX(10)');
      if (mode === 'css-transform') group.style.setProperty('transform', 'scale(0.9)');
      if (mode === 'viewBox') svg.setAttribute('viewBox', '0 0 200 100');
      if (mode === 'textLength') row.setAttribute('textLength', '30');
      if (mode === 'lengthAdjust') text.setAttribute('lengthAdjust', 'spacing');
      if (mode === 'rotate') row.setAttribute('rotate', '5');
      if (mode === 'multi-x') row.setAttribute('x', '0 10');
      if (mode === 'multi-y') row.setAttribute('y', '0 10');
      if (mode === 'multi-dy') row.setAttribute('dy', '0 10');
      if (mode === 'row-dx') row.setAttribute('dx', '1');
      if (mode === 'ancestor-dx') text.setAttribute('dx', '1');
      if (mode === 'stretch') text.style.setProperty('font-stretch', 'condensed');
      if (mode === 'caps') text.style.setProperty('font-variant-caps', 'small-caps');
      if (mode === 'features') text.style.setProperty('font-feature-settings', '"kern" 0');
      if (mode === 'variations') text.style.setProperty('font-variation-settings', '"wght" 700');
      if (mode === 'ligatures') text.style.setProperty('font-variant-ligatures', 'none');
      if (mode === 'mixed-text') row.appendChild(document.createTextNode('prefix'));
      if (mode === 'mixed-space') row.appendChild(document.createTextNode(' '));
      for (const word of ['A', ' system', ' of']) {
        const span = document.createElementNS(ns, 'tspan');
        span.setAttribute('class', 'text-inner-tspan');
        span.textContent = word;
        if (word === ' system' && mode === 'style') span.style.setProperty('letter-spacing', '1px');
        if (word === ' system' && mode === 'font') span.style.setProperty('font-weight', '700');
        if (word === ' system' && mode === 'position') span.setAttribute('dx', '1');
        if (word === ' system' && mode === 'direction') span.setAttribute('direction', 'rtl');
        row.appendChild(span);
      }
      text.appendChild(row);
      group.appendChild(text); svg.appendChild(group); document.documentElement.appendChild(svg);
      const before = calls;
      const box = text.getBBox();
      observed.push({ mode, called: calls > before, width: box.width });
      svg.remove();
    }
    const evidence = observed.map(value => `${value.mode}:${value.called}:${value.width}`).join(';');
    return { svg: '<svg data-fragment-observations="' + evidence + '"></svg>' };
  }
};
"##;

#[test]
fn c4_fragment_grouping_requires_plain_compatible_horizontal_spans() -> Result<(), String> {
    let rendered = render_svg_fixture(C4_FRAGMENT_OBSERVATION_FIXTURE)?;
    let rows = parse_observations(&rendered)?;
    assert_fragment_observations(&rows)?;
    Ok(())
}

fn parse_observations(rendered: &str) -> Result<Vec<FragmentObservation<'_>>, String> {
    let value = rendered
        .split_once("data-fragment-observations=\"")
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(value, _)| value)
        .ok_or("fragment observations missing")?;
    value
        .split(';')
        .map(|row| {
            let mut fields = row.split(':');
            let name = fields.next().ok_or("observation name missing")?;
            let called = fields
                .next()
                .ok_or("call flag missing")?
                .parse::<bool>()
                .map_err(|error| error.to_string())?;
            let width = fields
                .next()
                .ok_or("width missing")?
                .parse::<f64>()
                .map_err(|error| error.to_string())?;
            if fields.next().is_some() {
                return Err("extra observation field".to_owned());
            }
            Ok((name, called, width))
        })
        .collect()
}

const GROUPED_MODES: &[&str] = &["plain", "translation", "unsupported"];
const FALLBACK_MODES: &[&str] = &[
    "non-c4",
    "style",
    "font",
    "position",
    "direction",
    "word-spacing",
    "mixed-text",
    "mixed-space",
    "textLength",
    "lengthAdjust",
    "rotate",
    "multi-x",
    "multi-y",
    "multi-dy",
    "row-dx",
    "ancestor-dx",
    "stretch",
    "caps",
    "features",
    "variations",
    "ligatures",
    "scale",
    "rotation",
    "skew",
    "css-transform",
    "viewBox",
];

fn assert_fragment_observations(rows: &[FragmentObservation<'_>]) -> Result<(), String> {
    assert_eq!(rows.len(), GROUPED_MODES.len() + FALLBACK_MODES.len());
    assert_grouped_modes(rows)?;
    assert_fallback_modes(rows)?;
    assert_fragment_widths(rows)
}

fn assert_grouped_modes(rows: &[FragmentObservation<'_>]) -> Result<(), String> {
    for &name in GROUPED_MODES {
        assert!(observation(rows, name)?.1, "{name} must call bridge");
    }
    Ok(())
}

fn assert_fallback_modes(rows: &[FragmentObservation<'_>]) -> Result<(), String> {
    for &name in FALLBACK_MODES {
        assert!(!observation(rows, name)?.1, "{name} must not call bridge");
    }
    Ok(())
}

fn assert_fragment_widths(rows: &[FragmentObservation<'_>]) -> Result<(), String> {
    let plain = observation(rows, "plain")?.2;
    assert!(plain.is_finite() && plain > 0.0);
    assert_eq!(
        observation(rows, "unsupported")?.2,
        observation(rows, "non-c4")?.2
    );
    Ok(())
}

fn observation<'a>(
    rows: &[FragmentObservation<'a>],
    name: &str,
) -> Result<FragmentObservation<'a>, String> {
    let matches: Vec<_> = rows.iter().filter(|row| row.0 == name).collect();
    let [row] = matches.as_slice() else {
        return Err(format!("expected one observation: {name}"));
    };
    Ok(**row)
}
