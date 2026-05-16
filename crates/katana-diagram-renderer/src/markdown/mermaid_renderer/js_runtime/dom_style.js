function katanaStyleCamelName(name) {
  return String(name).replace(/-([a-z])/g, (_match, char) => char.toUpperCase());
}

function katanaStyleKebabName(name) {
  return String(name).replace(/[A-Z]/g, (char) => `-${char.toLowerCase()}`);
}

function katanaApplyCssText(style, value) {
  katanaCssEntries(value).forEach((entry) => {
    katanaApplyCssEntry(style, entry);
  });
}

function katanaCssEntries(value) {
  return String(value ?? "").split(";");
}

function katanaApplyCssEntry(style, entry) {
  const separator = entry.indexOf(":");
  if (separator < 0) return;
  style.setProperty(entry.slice(0, separator).trim(), entry.slice(separator + 1).trim());
}

[
  "backgroundColor",
  "border",
  "borderBottom",
  "borderColor",
  "borderRadius",
  "borderRightStyle",
  "color",
  "colorScheme",
  "cursor",
  "display",
  "fill",
  "fontFamily",
  "fontSize",
  "fontWeight",
  "height",
  "margin",
  "marginBottom",
  "maxHeight",
  "maxWidth",
  "opacity",
  "overflow",
  "padding",
  "paddingBottom",
  "position",
  "right",
  "stroke",
  "stopColor",
  "textAlign",
  "top",
  "verticalAlign",
  "width",
  "zIndex",
].forEach(katanaInstallStyleDirectProperty);

function katanaInstallStyleDirectProperty(camel) {
  if (Object.getOwnPropertyDescriptor(KatanaStyle.prototype, camel)) {
    return;
  }
  Object.defineProperty(KatanaStyle.prototype, camel, {
    get() {
      return this.getPropertyValue(katanaStyleKebabName(camel));
    },
    set(value) {
      this.setProperty(katanaStyleKebabName(camel), value);
    },
  });
}

KatanaStyle.prototype.setProperty = function setProperty(name, value) {
  const kebab = katanaStyleKebabName(name);
  const camel = katanaStyleCamelName(name);
  this.values[kebab] = String(value);
  this.values[camel] = String(value);
  if (!Object.getOwnPropertyDescriptor(KatanaStyle.prototype, camel)) {
    this[camel] = String(value);
  }
};

KatanaStyle.prototype.getPropertyValue = function getPropertyValue(name) {
  const key = String(name);
  const kebab = katanaStyleKebabName(key);
  const camel = katanaStyleCamelName(key);
  return this.values[key] ?? this.values[kebab] ?? this.values[camel] ?? katanaStyleOwnValue(this, camel);
};

function katanaStyleOwnValue(style, camel) {
  return Object.hasOwn(style, camel) ? style[camel] : "";
}

KatanaStyle.prototype.removeProperty = function removeProperty(name) {
  const value = this.getPropertyValue(name);
  delete this.values[String(name)];
  delete this.values[katanaStyleKebabName(name)];
  delete this.values[katanaStyleCamelName(name)];
  delete this[katanaStyleCamelName(name)];
  return value;
};

Object.defineProperty(KatanaStyle.prototype, "cssText", {
  get() {
    return Object.entries(this.values)
      .filter(([key]) => key.includes("-"))
      .map(([key, value]) => `${key}: ${value}`)
      .join("; ");
  },
  set(value) {
    this.values = {};
    katanaApplyCssText(this, value);
  },
});

const katanaSetAttributeBase = KatanaNode.prototype.setAttribute;
KatanaNode.prototype.setAttribute = function setAttribute(name, value) {
  katanaSetAttributeBase.call(this, name, value);
  if (String(name).toLowerCase() === "style") {
    this.style.cssText = value;
  }
};
