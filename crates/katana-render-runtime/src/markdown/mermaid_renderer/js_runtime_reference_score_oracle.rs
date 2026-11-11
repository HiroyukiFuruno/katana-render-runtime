#[cfg(target_os = "macos")]
use resvg::usvg::roxmltree::Document;

#[cfg(target_os = "macos")]
pub(super) const VIEW_BOX_COMPONENT_COUNT: usize = 4;
pub(in crate::markdown::mermaid_renderer::js_runtime) const SVG_DOM_PLACEMENT_SCALE: u32 = 64;

#[cfg(target_os = "macos")]
const VIEW_BOX_SERIALIZATION_TOLERANCE: f64 = 1.0 / SVG_DOM_PLACEMENT_SCALE as f64;

#[cfg(target_os = "macos")]
pub(super) const TIMELINE_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/21-01-timeline-phases.md");
#[cfg(target_os = "macos")]
pub(super) const TIMELINE_RAW_VIEW_BOX: [f64; VIEW_BOX_COMPONENT_COUNT] =
    [95.0, -61.0, 995.0, 534.400_024_414_062_5];
/* WHY: R977で別取得したofficial rawのviewBoxとSHAを固定し、更新済goldenへの追随を避ける。 */
#[cfg(target_os = "macos")]
pub(super) const TIMELINE_RAW_SOURCE_SHA256: &str =
    "9f91802978361be07fb4e837cf27bb917505af389fd9a46796c04f7fad1a4cbc";

const SVG_CSS_VARIABLE_FIXTURE: &str = r#"(() => {
      globalThis.__katanaMermaidDiagramType = 'gitGraph';
      const ns = 'http://www.w3.org/2000/svg';
      const parent = document.createElementNS(ns, 'g');
      parent.style.cssText = '--base: Noto Sans; --family: var(--base); font-family: var(--family)';
      document.body.appendChild(parent);
      const text = document.createElementNS(ns, 'text');
      text.style.cssText = '--base: serif; font-family: var(--family)';
      text.textContent = 'main';
      parent.appendChild(text);
      const verifyFamily = (family) => {
        const computed = getComputedStyle(text);
        if (computed.fontFamily !== family || computed.getPropertyValue('font-family') !== family) {
          throw new Error('computed font-family mismatch: ' + computed.fontFamily + ' / expected ' + family);
        }
        const expected = globalThis.__katanaMeasureSvgTextMetrics('main', family, 16, 400, false);
        const actual = text.getBBox();
        /* WHY: DOM配置幅は独立Chrome観測と同じ量子化で、生の計測値とは区別する。 */
        const expectedWidth = Math.ceil(expected.advance * SVG_DOM_PLACEMENT_SCALE) / SVG_DOM_PLACEMENT_SCALE;
        if (!(actual.width > 0) || actual.width !== expectedWidth || !(actual.height > 0)) {
          throw new Error('resolved SVG family did not use native metrics with DOM placement precision');
        }
      };
      verifyFamily('Noto Sans');
      text.style.cssText = '--a:var(--b) var(--c);--b:var(--a);--c:var(--b,serif);font-family:var(--a,var(--c,monospace))';
      verifyFamily('monospace');
      text.style.cssText = '--family:16px;font-family:var(--family,Arial)';
      verifyFamily('Noto Sans');
      parent.remove();
    })();"#;

#[cfg(target_os = "macos")]
pub(in super::super) fn mermaid_fixture_source(markdown: &str) -> Result<String, String> {
    let lines = markdown.lines().collect::<Vec<_>>();
    let start = lines
        .iter()
        .position(|line| line.trim_start().starts_with("~~~mermaid"))
        .ok_or("Timeline Mermaid fence missing")?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| line.trim() == "~~~")
        .map(|(index, _)| index)
        .ok_or("Timeline Mermaid fence unclosed")?;
    Ok(lines[start + 1..end].join("\n"))
}

#[cfg(target_os = "macos")]
pub(in super::super) fn assert_view_box_matches(
    actual: &str,
    expected: [f64; VIEW_BOX_COMPONENT_COUNT],
    source_sha256: &str,
) -> Result<(), String> {
    let actual = svg_view_box(actual)?;
    for (index, (actual, expected)) in actual.into_iter().zip(expected).enumerate() {
        let difference = (actual - expected).abs();
        if !difference.is_finite() || difference > VIEW_BOX_SERIALIZATION_TOLERANCE {
            return Err(format!(
                "SVG viewBox component {index} differs: {actual} vs {expected} (raw SHA-256 {source_sha256})"
            ));
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn svg_view_box(svg: &str) -> Result<[f64; VIEW_BOX_COMPONENT_COUNT], String> {
    let document = Document::parse(svg).map_err(|error| error.to_string())?;
    let value = document
        .root_element()
        .attribute("viewBox")
        .ok_or("SVG viewBox missing")?;
    let components = value
        .split_ascii_whitespace()
        .map(|component| component.parse::<f64>().map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    components
        .try_into()
        .map_err(|_| "Timeline viewBox must contain four components".to_owned())
}

pub(super) fn svg_css_variable_fixture() -> String {
    SVG_CSS_VARIABLE_FIXTURE.replace(
        "SVG_DOM_PLACEMENT_SCALE",
        &SVG_DOM_PLACEMENT_SCALE.to_string(),
    )
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::mermaid_fixture_source;

    #[test]
    fn fixture_source_rejects_missing_and_unclosed_fences() {
        assert_eq!(
            mermaid_fixture_source("timeline\n  section History"),
            Err("Timeline Mermaid fence missing".to_owned())
        );
        assert_eq!(
            mermaid_fixture_source("~~~mermaid\ntimeline\n  section History"),
            Err("Timeline Mermaid fence unclosed".to_owned())
        );
    }
}
