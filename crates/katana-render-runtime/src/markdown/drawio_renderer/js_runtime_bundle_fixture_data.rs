pub(super) const OFFICIAL_REFERENCE_VIEWPORT_BUNDLE_HOOK: &str = r#"  svg.setAttribute("viewBox", "0 0 1600 10");
  _container.appendChild(svg);
  svg.setAttribute("data-viewport", `${window.innerWidth}x${window.innerHeight}`);
  svg.setAttribute("data-initial-container", initialContainer);
  svg.setAttribute("data-constrained-container", `${_container.clientWidth}x${_container.clientHeight}`);
  _container.style.width = "1126px";
  _container.style.height = "665px";
  _container.style.minWidth = "100%";
  svg.setAttribute("data-min-width-container", `${_container.clientWidth}x${_container.clientHeight}`);
  _container.style.minWidth = "";
  svg.setAttribute("data-explicit-container", `${_container.clientWidth}x${_container.clientHeight}`);"#;

pub(super) const HTML_WRAP_BUNDLE_HOOK: &str = r#"  svg.appendChild(text);
  const foreignObject = document.createElementNS("http://www.w3.org/2000/svg", "foreignObject");
  const frame = document.createElement("div");
  frame.setAttribute("style", "display: flex; width: 198px; height: 1px;");
  const box = document.createElement("div");
  box.setAttribute("style", "box-sizing: border-box; font-size: 0;");
  const label = document.createElement("div");
  label.setAttribute(
    "style",
    "display: inline-block; font-size: 12px; font-family: Helvetica; line-height: 1.2; white-space: normal; word-wrap: normal;",
  );
  label.textContent = "Lorem ipsum dolor sit amet, consectetur adipisicing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.";
  box.appendChild(label);
  frame.appendChild(box);
  foreignObject.appendChild(frame);
  svg.appendChild(foreignObject);
  svg.setAttribute(
    "data-wrapped-client",
    `${box.clientWidth}x${box.clientHeight}:${label.clientWidth}x${label.clientHeight}`,
  );"#;

pub(super) const FAKE_BUNDLE_HEADER_PREFIX: &str = "\nfunction Graph() {}\n";

pub(super) const FAKE_BUNDLE_EDITOR_PREFIX: &str =
    "const Editor = {\n  convertHtmlToText(value) {\n";

pub(super) const FAKE_BUNDLE_VAR_EDITOR_PREFIX: &str =
    "var Editor = {\n  convertHtmlToText(value) {\n";

pub(super) const FAKE_BUNDLE_EDITOR_BODY: &str = "    return String(value);\n";

pub(super) const FAKE_BUNDLE_HEADER_MIDDLE: &str = r#"  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", ""#;

pub(super) const FAKE_BUNDLE_HEADER_HEIGHT: &str = r#"");
  svg.setAttribute("height", ""#;

pub(super) const FAKE_BUNDLE_VIEWBOX_PREFIX: &str = r#"");
  svg.setAttribute("viewBox", ""#;

pub(super) const FAKE_BUNDLE_VIEWBOX_SUFFIX: &str = "\");\n";

pub(super) const FAKE_BUNDLE_BODY: &str = r#"  const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
  text.textContent = "drawio";
  svg.appendChild(text);
"#;

pub(super) const FAKE_SHADOW_BODY: &str = r#"  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "cell");
  const shape = document.createElementNS("http://www.w3.org/2000/svg", "g");
  shape.setAttribute("transform", "translate(0.5,0.5)");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", "0");
  rect.setAttribute("y", "0");
  rect.setAttribute("width", "20");
  rect.setAttribute("height", "10");
  shape.appendChild(rect);
  group.appendChild(shape);
  svg.appendChild(group);
"#;

pub(super) const FAKE_BUNDLE_FOOTER: &str = r#"  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
"#;

pub(super) const FAKE_BUNDLE_WITH_HTML_COMMENT_LABEL_BODY: &str = r#"  const foreignObject = document.createElementNS("http://www.w3.org/2000/svg", "foreignObject");
  foreignObject.appendChild(document.createComment("hidden label"));
  const commentSwitch = document.createElementNS("http://www.w3.org/2000/svg", "switch");
  const artifactForeignObject = document.createElementNS(
    "http://www.w3.org/2000/svg",
    "foreignObject",
  );
  artifactForeignObject.textContent = "!-->";
  const artifactText = document.createElementNS("http://www.w3.org/2000/svg", "text");
  artifactText.textContent = "!-->";
  commentSwitch.appendChild(artifactForeignObject);
  commentSwitch.appendChild(artifactText);
  svg.appendChild(foreignObject);
  svg.appendChild(commentSwitch);
"#;

pub(super) const FAKE_BUNDLE_WITH_FOREIGN_OBJECT: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "20");
  svg.setAttribute("height", "10");
  svg.setAttribute("viewBox", "0 0 20 10");
  const foreignObject = document.createElementNS("http://www.w3.org/2000/svg", "foreignObject");
  foreignObject.setAttribute("width", "100%");
  foreignObject.setAttribute("height", "100%");
  const div = document.createElement("div");
  div.setAttribute("style", "color: light-dark(#000000, #ffffff)");
  div.textContent = "html label";
  div.appendChild(document.createElement("br"));
  div.appendChild(document.createElement("hr"));
  foreignObject.appendChild(div);
  svg.appendChild(foreignObject);
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
"#;

pub(super) const FAKE_BUNDLE_WITH_LIGHT_DARK_SVG_PAINT_BODY: &str = r#"  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("width", "20");
  rect.setAttribute("height", "10");
  rect.setAttribute(
    "style",
    "fill: light-dark(rgb(255, 255, 255), rgb(18, 18, 18)); stroke: light-dark(#000000, #ffffff)",
  );
  svg.appendChild(rect);
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  path.setAttribute("d", "M 0 0 L 20 10");
  path.setAttribute(
    "style",
    "fill: light-dark(white, #000000); stroke: light-dark(black, #000000)",
  );
  svg.appendChild(path);
"#;
