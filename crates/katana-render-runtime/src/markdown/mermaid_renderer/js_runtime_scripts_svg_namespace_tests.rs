use super::render_svg_fixture;

#[test]
fn html_parser_preserves_svg_and_foreign_object_namespaces() -> Result<(), String> {
    let rendered = render_svg_fixture(
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    const source = '<html><body><svg><g><rect></rect></g><foreignObject><div>HTML</div></foreignObject></svg></body></html>';
    const parsed = new DOMParser().parseFromString(source, 'text/html');
    const svg = parsed.querySelector('svg');
    const group = svg?.querySelector('g');
    const rect = group?.querySelector('rect');
    const foreignObject = svg?.querySelector('foreignObject');
    const html = foreignObject?.querySelector('div');
    return { svg: '<svg data-parser="' + [svg, group, rect, foreignObject].every((node) => node?.namespaceURI === 'http://www.w3.org/2000/svg') + '" data-html="' + (html?.namespaceURI === 'http://www.w3.org/1999/xhtml') + '"></svg>' };
  }
};
"##,
    )?;
    assert!(
        rendered.contains(r##"data-parser="true" data-html="true"##),
        "{rendered}"
    );
    Ok(())
}

#[test]
fn svg_inner_html_preserves_svg_context_and_foreign_object_html() -> Result<(), String> {
    let rendered = render_svg_fixture(
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    const group = document.createElementNS('http://www.w3.org/2000/svg', 'g');
    group.innerHTML = '<rect></rect><foreignObject><div><svg><g></g></svg></div></foreignObject>';
    const rect = group.querySelector('rect');
    const foreignObject = group.querySelector('foreignObject');
    const html = foreignObject?.querySelector('div');
    const nestedSvg = html?.querySelector('svg');
    const nestedGroup = nestedSvg?.querySelector('g');
    const foreignObjectInnerHtml = document.createElementNS('http://www.w3.org/2000/svg', 'foreignObject');
    foreignObjectInnerHtml.innerHTML = '<div>HTML</div><svg><g></g></svg>';
    const directHtml = foreignObjectInnerHtml.querySelector('div');
    const directSvg = foreignObjectInnerHtml.querySelector('svg');
    const directGroup = directSvg?.querySelector('g');
    return { svg: '<svg data-svg="' + [rect, foreignObject, nestedSvg, nestedGroup, directSvg, directGroup].every((node) => node?.namespaceURI === 'http://www.w3.org/2000/svg') + '" data-html="' + [html, directHtml].every((node) => node?.namespaceURI === 'http://www.w3.org/1999/xhtml') + '"></svg>' };
  }
};
"##,
    )?;
    assert!(
        rendered.contains(r##"data-svg="true" data-html="true"##),
        "{rendered}"
    );
    Ok(())
}
