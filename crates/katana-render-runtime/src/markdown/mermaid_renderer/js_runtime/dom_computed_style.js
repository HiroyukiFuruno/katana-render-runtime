globalThis.getComputedStyle = (node) => katanaComputedStyle(node);

function katanaComputedStyle(node) {
  const box = katanaComputedStyleBox(node);
  return {
    width: `${box.width}px`,
    height: `${box.height}px`,
    display: "block",
    fontSize: katanaComputedFontSize(node),
    get fontFamily() {
      return katanaComputedFontFamily(node);
    },
    getPropertyValue(name) {
      return katanaComputedPropertyValue(node, this, name);
    },
  };
}

function katanaComputedFontSize(node) {
  return katanaComputedStyleValue(node, "font-size") || "16px";
}

function katanaComputedFontFamily(node) {
  return katanaResolvedNodeTextFamily(node);
}

function katanaComputedPropertyValue(node, style, name) {
  if (name === "font-family") return style.fontFamily;
  return katanaComputedStyleValue(node, name) || style[name] || "";
}

function katanaComputedStyleBox(node) {
  if (node) {
    return katanaElementClientBox(node);
  }
  return katanaBox(0, 0, 0, 0);
}

function katanaComputedStyleValue(node, name) {
  return (
    [katanaCascadedStyleValue(node, name, katanaCssText(node))]
      .filter(Boolean)
      .concat([katanaDefaultComputedStyleValue(name)])[0] ?? ""
  );
}

function katanaDefaultComputedStyleValue(name) {
  if (/^(padding|margin|border).*/.test(name)) {
    return "0px";
  }
  return "";
}
