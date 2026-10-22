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
