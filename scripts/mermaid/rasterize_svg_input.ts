export class SvgRasterizeInput {
  constructor(private svg: string) {}

  shouldRasterize(): boolean {
    return /<svg(?:\s|>)/i.test(this.svg);
  }

  browserInnerHtml(): string {
    return this.svg.replace(SVG_HTML_VOID_ELEMENT_CLOSING_TAGS, "");
  }
}

const SVG_HTML_VOID_ELEMENT_CLOSING_TAGS = /<\/(?:br|hr)>/gi;
