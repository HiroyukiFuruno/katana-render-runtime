export class SvgRasterizeInput {
  constructor(private svg: string) {}

  shouldRasterize(): boolean {
    return /<svg(?:\s|>)/i.test(this.svg);
  }
}
