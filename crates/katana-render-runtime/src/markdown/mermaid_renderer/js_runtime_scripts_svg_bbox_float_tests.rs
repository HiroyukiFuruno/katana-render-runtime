use super::render_svg_fixture;

const SVG_GETBBOX_FLOAT_FIXTURE: &str = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    const ns = 'http://www.w3.org/2000/svg';
    const circle = document.createElementNS(ns, 'circle');
    for (const [name, value] of [['cx', '0'], ['cy', '0'], ['r', '42.16911261817187']]) {
      circle.setAttribute(name, value);
    }
    const group = document.createElementNS(ns, 'g');
    group.setAttribute('transform', 'translate(2.2 -1.1)');
    group.appendChild(circle);
    const root = document.createElementNS(ns, 'svg');
    root.appendChild(group);
    document.body.appendChild(root);
    const html = document.createElement('div');
    html.setAttribute('width', '9.438');
    html.setAttribute('height', '1');
    const circleBox = circle.getBBox();
    const groupBox = group.getBBox();
    const htmlBox = html.getBBox();
    const values = [circleBox, groupBox].flatMap((box) => [
      box.x,
      box.y,
      box.width,
      box.height,
    ]);
    const svgFloat = values.every((value) => value === Math.fround(value));
    const aliases = [circleBox, groupBox].every(
      (box) => box.width === box.w && box.height === box.h,
    );
    const htmlUnchanged = htmlBox.width === 9.438 && htmlBox.width !== Math.fround(htmlBox.width);
    const rawCircle = circleBox.x === -42.16911315917969 &&
      circleBox.width === 84.33822631835938 && groupBox.width === circleBox.width;
    return {
      svg:
        '<svg data-svg-float="' +
        svgFloat +
        '" data-aliases="' +
        aliases +
        '" data-raw-circle="' +
        rawCircle +
        '" data-html-unchanged="' +
        htmlUnchanged +
        '" data-html-ns="' +
        (html.namespaceURI === 'http://www.w3.org/1999/xhtml') +
        '"></svg>',
    };
  }
};
"##;

const EXPECTED_MARKERS: &str = r##"data-svg-float="true" data-aliases="true" data-raw-circle="true" data-html-unchanged="true" data-html-ns="true""##;

#[test]
fn svg_getbbox_uses_float_components_and_preserves_html_box() -> Result<(), String> {
    let rendered = render_svg_fixture(SVG_GETBBOX_FLOAT_FIXTURE)?;
    assert!(rendered.contains(EXPECTED_MARKERS), "{rendered}");
    Ok(())
}

const SVG_PATH_CACHE_FIXTURE: &str = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    const measure = globalThis.__katanaSvgPathBounds;
    if (typeof measure !== 'function') {
      const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
      path.setAttribute('d', 'M1,2 L11,7');
      const box = path.getBBox();
      return {svg: `<svg data-path-cache="${box.x === 1 && box.y === 2 && box.width === 10 && box.height === 5 ? 'fallback' : 'bad'}"></svg>`};
    }
    let calls = 0;
    globalThis.__katanaSvgPathBounds = (data) => { calls++; return measure(data); };
    const ns = 'http://www.w3.org/2000/svg';
    const root = document.createElementNS(ns, 'svg');
    const group = document.createElementNS(ns, 'g');
    const path = document.createElementNS(ns, 'path');
    group.setAttribute('transform', 'translate(3 4)');
    path.setAttribute('d', 'M1,2 L11,7');
    path.setAttribute('stroke', 'red');
    path.setAttribute('stroke-width', '100');
    group.appendChild(path); root.appendChild(group); document.body.appendChild(root);
    const first = path.getBBox(); path.getBBox();
    const transformed = root.getBBox();
    const initial = calls === 1 && first.width === 10 && first.height === 5 &&
      transformed.x === 4 && transformed.y === 6 && transformed.width === 10;
    path.setAttribute('d', 'M1,2 L13,7');
    const changed = path.getBBox().width === 12 && calls === 2;
    for (let index = 0; index < 256; index++) {
      path.setAttribute('d', `M${index + 1000},0 L${index + 1001},1`);
      path.getBBox();
    }
    path.setAttribute('d', 'M1,2 L11,7'); path.getBBox();
    const bounded = calls === 259;
    const large = 'M0,0' + ' L1,1'.repeat(1000);
    path.setAttribute('d', large);
    const beforeLarge = calls;
    const firstLarge = path.getBBox(); const secondLarge = path.getBBox();
    const largeMeasured = calls === beforeLarge + 2 && firstLarge.width === 1 && secondLarge.width === 1;
    path.setAttribute('d', 'M7,0 L8,1'); path.getBBox();
    for (let index = 0; index < 20; index++) {
      path.setAttribute('d', `M${index + 3000},0` + ' L1,1'.repeat(400));
      path.getBBox();
    }
    const beforeEvicted = calls;
    path.setAttribute('d', 'M7,0 L8,1'); path.getBBox();
    const byteEvicted = calls === beforeEvicted + 1;
    return {svg: `<svg data-path-cache="${initial && changed && bounded && largeMeasured && byteEvicted}"></svg>`};
  }
};
"##;

#[test]
fn svg_path_bounds_cache_is_bounded_and_invalidated_by_path_changes() -> Result<(), String> {
    let rendered = render_svg_fixture(SVG_PATH_CACHE_FIXTURE)?;
    #[cfg(target_os = "macos")]
    assert!(rendered.contains(r#"data-path-cache="true""#), "{rendered}");
    #[cfg(not(target_os = "macos"))]
    assert!(
        rendered.contains(r#"data-path-cache="fallback""#),
        "{rendered}"
    );
    Ok(())
}
