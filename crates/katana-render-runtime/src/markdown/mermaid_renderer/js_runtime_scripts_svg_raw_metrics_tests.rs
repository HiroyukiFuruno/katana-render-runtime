use super::render_svg_fixture;

const SVG_RUN_DEFAULT_FONT_SIZE_PX: f32 = 16.0;

const SVG_COMBINED_DEFAULTS_FIXTURE: &str = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    globalThis.__katanaMermaidDiagramType = 'classdiagram';
    const ns = 'http://www.w3.org/2000/svg';
    const cases = [['letter-spacing', '1px'], ['font-feature-settings', '"kern" 0'], ['font-variation-settings', '"wght" 700']];
    const valid = cases.every(([property, value]) => {
      const text = document.createElementNS(ns, 'text');
      const first = document.createElementNS(ns, 'tspan');
      const second = document.createElementNS(ns, 'tspan');
      first.textContent = 'A'; second.textContent = 'V';
      second.style.setProperty(property, value);
      text.appendChild(first); text.appendChild(second);
      const lines = katanaOrderedTextLines(text);
      const parts = lines.flatMap(line => line.parts);
      const raw = globalThis.__katanaMeasureSvgTextMetrics('A', 'trebuchet ms, verdana, arial, sans-serif', 16, 400, false);
      const rejectedCombined = parts.length === 2 && parts[0].svgMetrics !== undefined && parts[1].svgMetrics === undefined;
      const box = text.getBBox();
      return rejectedCombined && Math.abs(parts[0].width - raw.advance) < 0.0001 && Number.isFinite(box.width);
    });
    return { svg: '<svg data-all-run-defaults="' + valid + '"></svg>' };
  }
};
"##;

#[test]
fn real_svg_bridge_accepts_finite_zero_without_changing_css_boundary() -> Result<(), String> {
    let rendered = render_svg_fixture(
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    globalThis.__katanaMermaidDiagramType = 'classdiagram';
    const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
    text.style.setProperty('font-size', '0px');
    text.textContent = 'DB';
    const metrics = globalThis.__katanaMeasureSvgTextMetrics('DB', 'Noto Sans', 0, 400, false);
    const box = text.getBBox();
    const zero = [metrics.advance, metrics.ascent, metrics.descent].every(value => value === 0);
    const css = globalThis.__katanaMeasureText('DB', 'Noto Sans', 0, 400, false) === undefined;
    const geometry = box.width === 1 && box.height === 12 && text.getComputedTextLength() === 1;
    return { svg: '<svg data-zero-contract="' + [zero, css, geometry].join(',') + '"></svg>' };
  }
};
"##,
    )?;
    assert!(
        rendered.contains(r##"data-zero-contract="true,true,true"##),
        "{rendered}"
    );
    Ok(())
}

#[test]
fn svg_combined_raw_metrics_require_default_advance_properties_on_every_run() -> Result<(), String>
{
    let rendered = render_svg_fixture(SVG_COMBINED_DEFAULTS_FIXTURE)?;
    assert!(
        rendered.contains(r##"data-all-run-defaults="true"##),
        "{rendered}"
    );
    Ok(())
}

#[cfg(target_os = "macos")]
const SVG_PRIMARY_FONT_STRUT_FIXTURE: &str = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    const family = 'Trebuchet MS, Verdana, Arial, sans-serif';
    globalThis.__katanaMermaidDiagramType = 'classdiagram';
    const cases = [[400,"A",9.4375,19,9.4375],[400,"動物",32,19,32],[400,"+int 年齢",64.859375,19,64.859375],[700,"A",10.140625,19,10.140625],[700,"動物",32,19,32],[700,"+int 年齢",66.765625,19,66.765625]];
    const rows = cases.map(([weight, value, expectedWidth, expectedHeight, expectedLength]) => {
      const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
      text.style.setProperty('font-family', family);
      text.style.setProperty('font-size', '16px');
      text.style.setProperty('font-weight', String(weight));
      text.textContent = value;
      try {
        const metrics = globalThis.__katanaMeasureSvgTextMetrics(value, family, 16, weight, false);
        const box = text.getBBox();
        const length = text.getComputedTextLength();
        text.textContent = '';
        const span = document.createElementNS('http://www.w3.org/2000/svg', 'tspan');
        span.setAttribute('x', '0');
        span.setAttribute('y', '0');
        span.textContent = value;
        text.appendChild(span);
        const positionedBox = text.getBBox();
        return {
          family, weight, text: value, expectedWidth, expectedHeight, expectedLength,
          length, positionedLength: text.getComputedTextLength(),
          advance: metrics.advance, ascent: metrics.ascent, descent: metrics.descent,
          bboxWidth: box.width, bboxHeight: box.height,
          positionedWidth: positionedBox.width, positionedHeight: positionedBox.height,
        };
      } catch (error) {
        return { family, weight, text: value, error: String(error) };
      }
    });
    const cjk = rows.filter(row => row.text === '動物');
    const finite = rows.every(row => Number.isFinite(row.advance)
      && Number.isFinite(row.ascent) && Number.isFinite(row.descent)
      && Number.isFinite(row.bboxWidth) && Number.isFinite(row.bboxHeight)
      && Number.isFinite(row.positionedWidth) && Number.isFinite(row.positionedHeight));
    const primaryStrut = cjk.length === 2 && cjk.every(row => row.advance === 32
      && row.bboxHeight === 19 && row.positionedHeight === 19);
    const widthsStable = rows.every(row => Math.abs(row.bboxWidth - row.expectedWidth) < 0.0001
      && Math.abs(row.positionedWidth - row.expectedWidth) < 0.0001
      && row.bboxHeight === row.expectedHeight && row.positionedHeight === row.expectedHeight
      && Math.abs(row.length - row.expectedLength) < 0.0001
      && Math.abs(row.positionedLength - row.expectedLength) < 0.0001);
    const payload = JSON.stringify({rows, finite, primaryStrut, widthsStable});
    const escaped = payload.replaceAll('&', '&amp;').replaceAll('"', '&quot;');
    return { svg: '<svg data-font-primary-strut="' + (finite && primaryStrut) + '" data-font-primary-widths="' + widthsStable + '" data-font-primary-raw="' + escaped + '"></svg>' };
  }
};
"##;

#[cfg(target_os = "macos")]
#[test]
fn macos_svg_primary_strut_keeps_browser_linebox_for_fallback_cjk() -> Result<(), String> {
    let rendered = render_svg_fixture(SVG_PRIMARY_FONT_STRUT_FIXTURE)?;
    assert!(
        rendered.contains(r##"data-font-primary-strut="true" data-font-primary-widths="true""##),
        "native SVG primary-strut measurement: {rendered}"
    );
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn macos_svg_glyph_overhang_matches_chromium_after_advance_cache_hit() -> Result<(), String> {
    let rendered = render_svg_fixture(CHROMIUM_INK_CACHE_FIXTURE)?;
    assert!(
        rendered.contains(r##"data-chromium-ink="true""##),
        "native SVG ink bounds differ from measured Chromium geometry: {rendered}"
    );
    Ok(())
}

pub(super) fn svg_fixture_with_independent_raw_advances(
    fixture: &str,
    runs: &[(&str, u16)],
) -> Result<String, String> {
    use crate::markdown::svg_rasterize::SvgRasterizeOps;

    let advances = runs
        .iter()
        .map(|(text, weight)| {
            SvgRasterizeOps::measure_svg_text_metrics(
                text,
                "trebuchet ms, verdana, arial, sans-serif",
                SVG_RUN_DEFAULT_FONT_SIZE_PX,
                *weight,
                false,
            )
            .map(|metrics| metrics.advance)
            .map_err(|error| format!("independent Rust SVG run failed: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let json = serde_json::to_string(&advances).map_err(|error| error.to_string())?;
    Ok(fixture.replace("__KATANA_RUST_RAW_SVG_ADVANCES__", &json))
}

#[cfg(target_os = "macos")]
const CHROMIUM_INK_CACHE_FIXTURE: &str = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    globalThis.__katanaMermaidDiagramType = 'classdiagram';
    const cases = [[400,"API Application",-0.0234375,-14,107.6484375,17.1796875,107.625],[400,"API",-0.0234375,-14,25.8203125,17,25.796875],[400,"Application",-0.0234375,-14,78.3046875,17.1796875,78.28125],[400,"iii",0,-14,10.671875,17,10.671875],[400,"AV",-0.0234375,-14,20.1796875,17,20.15625],[700,"API Application",0,-14,117.640625,17.15625,117.640625],[700,"API",0,-14,26.671875,17,26.671875],[700,"Application",0,-14,87.109375,17.15625,87.109375],[700,"iii",0,-14,13.34375,17,13.34375],[700,"AV",0,-14,21.046875,17,21.046875]];
    const rows = cases.map(([weight, value, left, top, width, height, length]) => {
      globalThis.__katanaMeasureText(value, 'Arial', 16, weight, false);
      const metrics = globalThis.__katanaMeasureSvgTextMetrics(value, 'Arial', 16, weight, false);
      const element = document.createElementNS('http://www.w3.org/2000/svg', 'text');
      element.style.setProperty('font-family', 'Arial');
      element.style.setProperty('font-size', '16px');
      element.style.setProperty('font-weight', String(weight));
      element.textContent = value;
      const plain = element.getBBox();
      const advance = element.getComputedTextLength();
      element.textContent = '';
      const positioned = document.createElementNS(element.namespaceURI, 'tspan');
      positioned.setAttribute('x', '20');
      positioned.setAttribute('y', '30');
      positioned.textContent = value;
      element.appendChild(positioned);
      const shifted = element.getBBox();
      const expected = [left, top, width, height, left + 20, top + 30, width, height];
      const actual = [plain.x, plain.y, plain.width, plain.height,
        shifted.x, shifted.y, shifted.width, shifted.height];
      return {weight, value, actual, expected, advance, length, rawAdvance: metrics.advance,
        matches: actual.every((item, index) => Math.abs(item - expected[index]) < 0.0001)
          && Math.abs(advance - length) < 0.0001
          && metrics.advance <= length && length - metrics.advance < 1 / 64};
    });
    const payload = JSON.stringify(rows).replaceAll('&', '&amp;').replaceAll('"', '&quot;');
    return {svg: '<svg data-chromium-ink="' + rows.every(row => row.matches)
      + '" data-chromium-ink-rows="' + payload + '"></svg>'};
  }
};
"##;
