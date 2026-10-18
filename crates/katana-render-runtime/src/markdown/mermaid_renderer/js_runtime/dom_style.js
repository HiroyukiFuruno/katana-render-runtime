function katanaStyleCamelName(name) {
  if (String(name).startsWith("--")) return String(name);
  return String(name).replace(/-([a-z])/g, (_match, char) => char.toUpperCase());
}

function katanaStyleKebabName(name) {
  if (String(name).startsWith("--")) return String(name);
  return String(name).replace(/[A-Z]/g, (char) => `-${char.toLowerCase()}`);
}

function katanaApplyCssText(style, value) {
  katanaCssEntries(value).forEach((entry) => {
    katanaApplyCssEntry(style, entry);
  });
}

function katanaCssEntries(value) {
  return globalThis.katanaParseCssDeclarations(String(value ?? ""));
}

function katanaApplyCssEntry(style, entry) {
  if (style.getPropertyPriority(entry.name) === "important" && !entry.priority) return;
  style.setProperty(entry.name, entry.value, entry.priority);
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

KatanaStyle.prototype.setProperty = function setProperty(name, value, priority = "") {
  const kebab = katanaStyleKebabName(name);
  const camel = katanaStyleCamelName(name);
  const normalizedPriority = String(priority).toLowerCase();
  if (value === null || value === undefined || String(value) === "") {
    this.removeProperty(name);
    return;
  }
  if (normalizedPriority !== "" && normalizedPriority !== "important") return;
  if (kebab === "font-family") {
    const family = String(value);
    const references = globalThis.katanaCssVariableReferences(family);
    if (
      references === undefined ||
      (references.length === 0 && !globalThis.katanaValidCssFontFamily(family))
    )
      return;
  }
  this.priorities ??= {};
  this.priorities[kebab] = normalizedPriority;
  this.values[kebab] = String(value);
  this.values[camel] = String(value);
  if (!Object.getOwnPropertyDescriptor(KatanaStyle.prototype, camel)) {
    this[camel] = String(value);
  }
};

KatanaStyle.prototype.getPropertyPriority = function getPropertyPriority(name) {
  return this.priorities?.[katanaStyleKebabName(name)] ?? "";
};

KatanaStyle.prototype.clone = function clone() {
  const cloned = new KatanaStyle();
  cloned.values = { ...this.values };
  cloned.priorities = { ...this.priorities };
  return cloned;
};

KatanaStyle.prototype.getPropertyValue = function getPropertyValue(name) {
  const key = String(name);
  const kebab = katanaStyleKebabName(key);
  const camel = katanaStyleCamelName(key);
  return (
    this.values[key] ?? this.values[kebab] ?? this.values[camel] ?? katanaStyleOwnValue(this, camel)
  );
};

function katanaStyleOwnValue(style, camel) {
  return Object.hasOwn(style, camel) ? style[camel] : "";
}

KatanaStyle.prototype.removeProperty = function removeProperty(name) {
  const value = this.getPropertyValue(name);
  delete this.values[String(name)];
  delete this.values[katanaStyleKebabName(name)];
  delete this.values[katanaStyleCamelName(name)];
  if (this.priorities) delete this.priorities[katanaStyleKebabName(name)];
  delete this[katanaStyleCamelName(name)];
  return value;
};

Object.defineProperty(KatanaStyle.prototype, "cssText", {
  get() {
    return Object.entries(this.values)
      .filter(([key]) => key.startsWith("--") || !/[A-Z]/.test(key))
      .map(
        ([key, value]) => `${key}: ${value}${this.getPropertyPriority(key) ? " !important" : ""}`,
      )
      .join("; ");
  },
  set(value) {
    for (const key of Object.keys(this.values)) this.removeProperty(key);
    this.values = {};
    this.priorities = {};
    katanaApplyCssText(this, value);
  },
});

Object.defineProperty(KatanaStyle.prototype, "length", {
  get() {
    return Object.keys(this.values).filter((key) => key.startsWith("--") || !/[A-Z]/.test(key))
      .length;
  },
});

KatanaStyle.prototype.item = function item(index) {
  return (
    Object.keys(this.values).filter((key) => key.startsWith("--") || !/[A-Z]/.test(key))[
      Number(index)
    ] ?? ""
  );
};

const katanaSetAttributeBase = KatanaNode.prototype.setAttribute;
KatanaNode.prototype.setAttribute = function setAttribute(name, value) {
  katanaSetAttributeBase.call(this, name, value);
  if (String(name).toLowerCase() === "style") {
    this.style.cssText = value;
  }
};
