use super::render_svg_fixture;

#[test]
fn svg_style_declaration_mutations_reach_serialized_node_and_clone() -> Result<(), String> {
    let rendered = render_svg_fixture(STYLE_DECLARATION_MUTATION_FIXTURE)?;
    assert_style_declaration_mutation_contract(&rendered);
    Ok(())
}

const STYLE_DECLARATION_MUTATION_FIXTURE: &str = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    const node = document.createElementNS('http://www.w3.org/2000/svg', 'text');
    node.setAttribute('style', 'font-size:28px;fill:lightgrey;text-anchor:middle');
    const initialFontSize = node.style.getPropertyValue('font-size');
    const initialAnchor = node.style.getPropertyValue('text-anchor');
    node.style.setProperty('font-size', '17px');
    node.style.setProperty('fill', 'red', 'important');
    const removedAnchor = node.style.removeProperty('text-anchor');
    const clone = node.cloneNode(true);
    clone.style.setProperty('fill', 'blue');
    const removed = node.cloneNode(true);
    removed.setAttribute('data-keep', 'yes');
    removed.removeAttribute('style');
    const styleState = (value) => {
      const declarations = Object.fromEntries(
        globalThis.katanaParseCssDeclarations(value).map((entry) => [
          entry.name,
          {value: entry.value, priority: entry.priority},
        ]),
      );
      return {
        fontSize: declarations['font-size']?.value || '',
        fill: declarations.fill?.value || '',
        fillPriority: declarations.fill?.priority || '',
        anchor: declarations['text-anchor']?.value || '',
      };
    };
    const attributeState = styleState(node.getAttribute('style') || '');
    const serializedStyle = /<text[^>]*style="([^"]*)"/.exec(node.outerHTML)?.[1] || '';
    const serializedState = styleState(serializedStyle);
    const cloneState = styleState(clone.getAttribute('style') || '');
    const removedState = styleState(removed.getAttribute('style') || '');
    const attributes = {
      'initial-font-size': initialFontSize,
      'initial-anchor': initialAnchor,
      'removed-property-anchor': removedAnchor,
      'attribute-font-size': attributeState.fontSize,
      'attribute-fill': attributeState.fill,
      'attribute-priority': attributeState.fillPriority,
      'attribute-anchor': attributeState.anchor,
      'serialized-font-size': serializedState.fontSize,
      'serialized-fill': serializedState.fill,
      'serialized-priority': serializedState.fillPriority,
      'serialized-anchor': serializedState.anchor,
      'clone-font-size': cloneState.fontSize,
      'clone-fill': cloneState.fill,
      'clone-priority': cloneState.fillPriority,
      'clone-anchor': cloneState.anchor,
      'removed-has-style': removed.hasAttribute('style'),
      'removed-font-size': removedState.fontSize,
      'removed-fill': removedState.fill,
      'removed-priority': removedState.fillPriority,
      'removed-anchor': removedState.anchor,
      'removed-outer-has-style': /<text[^>]*style=/.test(removed.outerHTML),
      'removed-outer-keeps-attribute': removed.outerHTML.includes('data-keep="yes'),
    };
    const serializedAttributes = Object.entries(attributes)
      .map(([name, value]) => ' data-' + name + '="' + value + '"').join('');
    return {svg: '<svg' + serializedAttributes + '>' + node.outerHTML + '</svg>'};
  }
};
"##;

fn assert_style_declaration_mutation_contract(rendered: &str) {
    for expected in [
        r#"data-initial-font-size="28px""#,
        r#"data-initial-anchor="middle""#,
        r#"data-removed-property-anchor="middle""#,
        r#"data-attribute-font-size="17px""#,
        r#"data-attribute-fill="red""#,
        r#"data-attribute-priority="important""#,
        r#"data-attribute-anchor="""#,
        r#"data-serialized-font-size="17px""#,
        r#"data-serialized-fill="red""#,
        r#"data-serialized-priority="important""#,
        r#"data-serialized-anchor="""#,
        r#"data-clone-font-size="17px""#,
        r#"data-clone-fill="blue""#,
        r#"data-clone-priority="""#,
        r#"data-clone-anchor="""#,
        r#"data-removed-has-style="false""#,
        r#"data-removed-font-size="""#,
        r#"data-removed-fill="""#,
        r#"data-removed-priority="""#,
        r#"data-removed-anchor="""#,
        r#"data-removed-outer-has-style="false""#,
        r#"data-removed-outer-keeps-attribute="true""#,
    ] {
        assert!(rendered.contains(expected), "{expected}: {rendered}");
    }
}
