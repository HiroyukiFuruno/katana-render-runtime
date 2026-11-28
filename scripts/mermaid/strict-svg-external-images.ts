import type { PageHandle } from "./official-renderer-types";

export interface SvgImageHrefAttributes {
  href: string | null;
  xlinkHref: string | null;
}

export interface LoadedSvgExternalImage {
  url: string;
  complete: boolean;
  naturalWidth: number;
  naturalHeight: number;
  decoded: boolean;
}

export function strictSvgExternalImageUrls(references: SvgImageHrefAttributes[]): string[] {
  const urls = references.map(({ href, xlinkHref }, index) => {
    const url = href === null ? xlinkHref : href;
    if (url === null || url.trim() === "") {
      throw new Error(`Strict SVG image ${index} has no usable href`);
    }
    return url;
  });
  return [...new Set(urls)];
}

export function assertLoadedSvgExternalImages(images: LoadedSvgExternalImage[]) {
  for (const image of images) {
    if (
      image.url.trim() === "" ||
      !image.complete ||
      !image.decoded ||
      !Number.isFinite(image.naturalWidth) ||
      !Number.isFinite(image.naturalHeight) ||
      image.naturalWidth <= 0 ||
      image.naturalHeight <= 0
    ) {
      throw new Error(`Strict SVG image did not load and decode: ${image.url}`);
    }
  }
}

export async function waitForStrictSvgExternalImages(
  page: Pick<PageHandle, "evaluate">,
): Promise<void> {
  const references = await page.evaluate(() => {
    const namespace = "http://www.w3.org/1999/xlink";
    return Array.from(document.querySelectorAll("#diagram image")).map((image) => ({
      href: image.getAttribute("href"),
      xlinkHref: image.getAttributeNS(namespace, "href") ?? image.getAttribute("xlink:href"),
    }));
  });
  const urls = strictSvgExternalImageUrls(references);
  const loaded = await page.evaluate(async (imageUrls) => {
    const loadAndDecode = (url: string) =>
      new Promise<{
        url: string;
        complete: boolean;
        naturalWidth: number;
        naturalHeight: number;
        decoded: boolean;
      }>((resolve, reject) => {
        const image = new Image();
        let decoding = false;
        const fail = () => reject(new Error(`Strict SVG image failed to load: ${url}`));
        const decodeIfComplete = () => {
          if (!image.complete || decoding) return;
          decoding = true;
          if (image.naturalWidth <= 0 || image.naturalHeight <= 0) {
            fail();
            return;
          }
          image.decode().then(
            () =>
              resolve({
                url,
                complete: image.complete,
                naturalWidth: image.naturalWidth,
                naturalHeight: image.naturalHeight,
                decoded: true,
              }),
            fail,
          );
        };
        image.addEventListener("load", decodeIfComplete, { once: true });
        image.addEventListener("error", fail, { once: true });
        image.src = url;
        decodeIfComplete();
      });
    const images = await Promise.all(imageUrls.map(loadAndDecode));
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    return images;
  }, urls);
  assertLoadedSvgExternalImages(loaded);
}
