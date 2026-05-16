function katanaInstallDrawioRuntimeAdapter() {
  globalThis.__katanaDrawioMissingResources = [];
  globalThis.__katanaDrawioResourceErrors = [];
  const context = katanaDrawioRuntimeContext();
  katanaRegisterDrawioResources(context);
  katanaInstallDrawioStencilLoader(context);
  katanaInstallDrawioMxUtils(context);
  katanaInstallDrawioLightDarkColorSupport();
  katanaInstallDrawioTextConversionGuard();
  katanaInstallDrawioSvgTextOutput();
  katanaInstallDrawioImageBundles(context);
  katanaInstallDrawioSvgImages(context);
  katanaInstallDrawioOwnerSvgElement();
}

katanaInstallDrawioRuntimeAdapter();

function katanaInstallDrawioTextConversionGuard() {
  const originalConvertHtmlToText = Editor.convertHtmlToText;
  Editor.convertHtmlToText = function convertHtmlToText(value) {
    return KATANA_DRAWIO_TEXT_CONVERSION_HANDLERS[Number(value != null)](
      originalConvertHtmlToText,
      value,
    );
  };
}

function katanaInstallDrawioLightDarkColorSupport() {
  const runtimeMxUtils = katanaDrawioMxUtils();
  if (typeof runtimeMxUtils === "object") {
    runtimeMxUtils.lightDarkColorSupported = true;
    runtimeMxUtils.preferDarkColor = false;
  }
}

function katanaDrawioMxUtils() {
  try {
    return mxUtils;
  } catch (_error) {
    return globalThis.mxUtils;
  }
}

const KATANA_DRAWIO_TEXT_CONVERSION_HANDLERS = [
  () => "",
  (originalConvertHtmlToText, value) => originalConvertHtmlToText.call(Editor, value),
];

function katanaDrawioHtmlLabelText(value) {
  const html = String(value);
  const normalized = KATANA_DRAWIO_HTML_LABEL_NORMALIZERS[Number(katanaCanUseDomParser())](html);
  return katanaNormalizeHtmlLabelWhitespace(normalized);
}

function katanaCanUseDomParser() {
  return typeof document === "object" && typeof document.createElement === "function";
}

const KATANA_DRAWIO_HTML_LABEL_NORMALIZERS = [
  (html) => katanaNormalizeHtmlLabelTextByRegex(html),
  (html) => katanaNormalizeHtmlLabelTextByDom(html),
];

function katanaNormalizeHtmlLabelTextByRegex(html) {
  return decodeHtmlEntities(
    html
      .replace(/<\s*br\b[^>]*>/gi, "\n")
      .replace(/<\s*\/\s*(div|p|li|tr|td|th|h[1-6]|section|article)\s*>/gi, "\n")
      .replace(/<[^>]+>/g, ""),
  );
}

function katanaNormalizeHtmlLabelTextByDom(html) {
  const container = document.createElement("div");
  container.innerHTML = html;
  return katanaCollectHtmlLabelTextFromNode(container).trim();
}

function katanaCollectHtmlLabelTextFromNode(root) {
  const buffer = [];
  const inlineBreakTags = new Set(["BR"]);
  const blockTags = new Set([
    "DIV",
    "P",
    "LI",
    "UL",
    "OL",
    "TABLE",
    "TBODY",
    "THEAD",
    "TFOOT",
    "TR",
    "TD",
    "TH",
    "SECTION",
    "ARTICLE",
    "BLOCKQUOTE",
    "PRE",
    "H1",
    "H2",
    "H3",
    "H4",
    "H5",
    "H6",
  ]);
  const visit = (node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      buffer.push(node.nodeValue ?? "");
      return;
    }
    if (node.nodeType !== Node.ELEMENT_NODE) {
      return;
    }
    const element = node;
    if (inlineBreakTags.has(element.tagName)) {
      buffer.push("\n");
      return;
    }
    if (blockTags.has(element.tagName) && element !== root) {
      if (buffer.length > 0 && !String(buffer[buffer.length - 1]).endsWith("\n")) {
        buffer.push("\n");
      }
    }
    Array.from(element.childNodes).forEach(visit);
  };
  visit(root);
  return decodeHtmlEntities(buffer.join(""));
}

function katanaNormalizeHtmlLabelWhitespace(text) {
  return text
    .replace(/\u00a0/g, " ")
    .replace(/[ \t\f\v]+/g, " ")
    .replace(/ *\n */g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function katanaInstallDrawioSvgTextOutput() {
  katanaDisableDrawioForeignObjectWarning();
}

function katanaDisableDrawioForeignObjectWarning() {
  if (typeof Graph === "function") {
    Graph.prototype.addForeignObjectWarning = function addForeignObjectWarning() {};
  }
}
