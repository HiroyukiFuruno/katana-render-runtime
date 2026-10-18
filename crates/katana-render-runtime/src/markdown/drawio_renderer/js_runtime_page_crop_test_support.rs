macro_rules! create_rect_group_js {
    () => {
        r#"function createRectGroup(id, x, y, width, height) {
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", id);
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", String(x));
  rect.setAttribute("y", String(y));
  rect.setAttribute("width", String(width));
  rect.setAttribute("height", String(height));
  group.appendChild(rect);
  return group;
}
"#
    };
}

pub(super) use create_rect_group_js;

pub(super) fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

pub(super) const FAKE_BUNDLE_WITH_NEGATIVE_DISABLED_PAGE_BOUNDS: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "1571px");
  svg.setAttribute("height", "512px");
  svg.setAttribute("viewBox", "0 0 1571 512");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "shape");
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  path.setAttribute("d", "M 121 -62 L 123 184");
  group.appendChild(path);
  svg.appendChild(group);
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
"#;

pub(super) const FAKE_BUNDLE_WITH_POSITIVE_TOP_PADDING: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "101px");
  svg.setAttribute("height", "300px");
  svg.setAttribute("viewBox", "0 0 101 300");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "shape");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", "0");
  rect.setAttribute("y", "10");
  rect.setAttribute("width", "100");
  rect.setAttribute("height", "100");
  group.appendChild(rect);
  svg.appendChild(group);
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
"#;

pub(super) const FAKE_BUNDLE_WITH_LEFT_TEXT_OVERFLOW: &str = concat!(
    r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "300px");
  svg.setAttribute("height", "100px");
  svg.setAttribute("viewBox", "0 0 300 100");
  svg.appendChild(createRectGroup("shape", 0, 0, 100, 50));
  const label = document.createElementNS("http://www.w3.org/2000/svg", "g");
  label.setAttribute("data-cell-id", "label");
  const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
  text.setAttribute("x", "-20");
  text.setAttribute("y", "15");
  text.textContent = "Label";
  label.appendChild(text);
  svg.appendChild(label);
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
"#,
    create_rect_group_js!(),
    r#""#
);

pub(super) const FAKE_BUNDLE_WITH_WIDE_WHITE_RECTANGLES: &str = r##"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "1410px");
  svg.setAttribute("height", "100px");
  svg.setAttribute("viewBox", "0 0 1410 100");
  svg.appendChild(createWideRectangleGroup("bar", 10));
  svg.appendChild(createWideRectangleGroup("label", 30));
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
function createWideRectangleGroup(id, y) {
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", id);
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", "5");
  rect.setAttribute("y", String(y));
  rect.setAttribute("width", "1400");
  rect.setAttribute("height", "10");
  rect.setAttribute("fill", "#ffffff");
  rect.setAttribute("stroke", "none");
  group.appendChild(rect);
  return group;
}
"##;

pub(super) const FAKE_BUNDLE_WITH_RENDERED_OVERFLOW: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "2560px");
  svg.setAttribute("height", "780px");
  svg.setAttribute("viewBox", "-560 -490 2560 780");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "phone");
  const phone = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  phone.setAttribute("x", "-560");
  phone.setAttribute("y", "-490");
  phone.setAttribute("width", "390");
  phone.setAttribute("height", "780");
  group.appendChild(phone);
  const overflow = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  overflow.setAttribute("x", "1000");
  overflow.setAttribute("y", "-490");
  overflow.setAttribute("width", "1000");
  overflow.setAttribute("height", "10");
  svg.appendChild(group);
  svg.appendChild(overflow);
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
"#;
