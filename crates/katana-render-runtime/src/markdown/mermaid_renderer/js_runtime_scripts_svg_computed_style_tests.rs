use super::render_svg_fixture;

const COMPUTED_STYLE_LAZY_FIXTURE: &str = r##"
globalThis.mermaid = {
  initialize() {},
  render: async () => {
    const group = document.createElementNS('http://www.w3.org/2000/svg', 'g');
    const rect = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
    group.setAttribute('width', '20');
    group.setAttribute('height', '10');
    rect.setAttribute('width', '20');
    rect.setAttribute('height', '10');
    group.appendChild(rect);
    let bboxCalls = 0;
    const originalGetBBox = SVGElement.prototype.getBBox;
    SVGElement.prototype.getBBox = function(...args) {
      bboxCalls += 1;
      return originalGetBBox.apply(this, args);
    };
    const computed = getComputedStyle(group);
    const afterCreation = bboxCalls;
    const fontFamily = computed.fontFamily;
    const fontSize = computed.fontSize;
    const afterFontReads = bboxCalls;
    const firstWidth = computed.width;
    const firstHeight = computed.height;
    const firstWidthProperty = computed.getPropertyValue('width');
    const firstHeightProperty = computed.getPropertyValue('height');
    const afterFirstDimensions = bboxCalls;
    group.setAttribute('width', '40');
    group.setAttribute('height', '30');
    const secondWidth = computed.width;
    const secondHeight = computed.height;
    const secondWidthProperty = computed.getPropertyValue('width');
    const secondHeightProperty = computed.getPropertyValue('height');
    const afterSecondDimensions = bboxCalls;
    SVGElement.prototype.getBBox = originalGetBBox;
    return { svg: '<svg data-no-layout="' + (afterCreation === 0 && afterFontReads === 0) + '" data-widths="' + [firstWidth, secondWidth].join(',') + '" data-heights="' + [firstHeight, secondHeight].join(',') + '" data-properties="' + (firstWidth === firstWidthProperty && firstHeight === firstHeightProperty && secondWidth === secondWidthProperty && secondHeight === secondHeightProperty) + '" data-bbox-increased="' + (afterFirstDimensions > afterFontReads && afterSecondDimensions > afterFirstDimensions) + '" data-font-types="' + (typeof fontFamily === 'string' && typeof fontSize === 'string') + '"></svg>' };
  }
};
"##;

#[test]
fn computed_style_font_reads_are_lazy_and_dimensions_stay_live() -> Result<(), String> {
    let rendered = render_svg_fixture(COMPUTED_STYLE_LAZY_FIXTURE)?;
    for expected in [
        r##"data-no-layout="true"##,
        r##"data-widths="20px,40px"##,
        r##"data-heights="10px,30px"##,
        r##"data-properties="true"##,
        r##"data-bbox-increased="true"##,
        r##"data-font-types="true"##,
    ] {
        assert!(rendered.contains(expected), "{expected}: {rendered}");
    }
    Ok(())
}
