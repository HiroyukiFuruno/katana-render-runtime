const KATANA_EMPTY_DRAWIO_ATTRIBUTES = new Map();
const KATANA_DRAWIO_ATTRIBUTE_READERS = [
  katanaEmptyDrawioAttributes,
  katanaReadableDrawioAttributes,
];
const KATANA_DRAWIO_SOURCE_OBJECT_ATTRIBUTES = katanaDrawioSourceObjectAttributes();
const KATANA_DRAWIO_SOURCE_VARS = katanaDrawioSourceVars();

function katanaDrawioSourceLabelEntries() {
  return [
    ...katanaDrawioSourceMxCellLabelEntries(),
    ...katanaDrawioSourceObjectLabelEntries(),
  ].filter(katanaHasDrawioCellLabel);
}

function katanaDrawioSourceMxCellLabelEntries() {
  return Array.from(katanaDrawioRequestSource().matchAll(/<mxCell\b([^>]*)>/g))
    .map(katanaDrawioMxCellLabelEntry);
}

function katanaDrawioMxCellLabelEntry(match) {
  const attributes = katanaDrawioXmlAttributes(match[1]);
  const value = katanaDrawioCellAttribute(attributes, "value");
  return katanaDrawioSourceLabelEntry(
    attributes,
    attributes,
    katanaDrawioResolvedLabelValue(attributes, attributes, value),
  );
}

function katanaDrawioSourceObjectLabelEntries() {
  return Array.from(
    katanaDrawioRequestSource().matchAll(/<(?:UserObject|object)\b([^>]*)>\s*<mxCell\b([^>]*)/g),
  ).map(katanaDrawioObjectLabelEntry);
}

function katanaDrawioObjectLabelEntry(match) {
  const objectAttributes = katanaDrawioXmlAttributes(match[1]);
  const cellAttributes = katanaDrawioXmlAttributes(match[2]);
  const value = katanaDrawioCellAttribute(objectAttributes, "label");
  return katanaDrawioSourceLabelEntry(
    objectAttributes,
    cellAttributes,
    katanaDrawioResolvedLabelValue(objectAttributes, cellAttributes, value),
  );
}

function katanaDrawioSourceLabelEntry(attributes, cellAttributes, value) {
  return {
    id: katanaDrawioCellAttribute(attributes, "id") || katanaDrawioCellAttribute(cellAttributes, "id"),
    value,
    label: katanaDrawioHtmlLabelText(value),
    edge: katanaDrawioCellAttribute(cellAttributes, "edge") === "1",
    style: katanaDrawioCellAttribute(cellAttributes, "style"),
  };
}

function katanaDrawioResolvedLabelValue(attributes, cellAttributes, value) {
  return katanaDrawioShouldResolvePlaceholders(attributes, cellAttributes)
    ? String(value).replace(/%([a-zA-Z0-9:_-]+)%/g, (_match, name) =>
        katanaDrawioPlaceholderValue(name, attributes, cellAttributes),
      )
    : value;
}

function katanaDrawioShouldResolvePlaceholders(attributes, cellAttributes) {
  return [
    katanaDrawioCellAttribute(attributes, "placeholders") === "1",
    katanaDrawioStyleValue(katanaDrawioCellAttribute(cellAttributes, "style"), "placeholders") === "1",
  ].some(Boolean);
}

function katanaDrawioPlaceholderValue(name, attributes, cellAttributes) {
  return [
    katanaDrawioCellAttribute(attributes, name),
    katanaDrawioCellAttribute(cellAttributes, name),
    katanaDrawioCellAttribute(katanaDrawioParentObjectAttributes(cellAttributes), name),
    KATANA_DRAWIO_SOURCE_VARS.get(name),
    `%${name}%`,
  ].filter(Boolean)[0];
}

function katanaDrawioParentObjectAttributes(cellAttributes) {
  return (
    KATANA_DRAWIO_SOURCE_OBJECT_ATTRIBUTES.get(katanaDrawioCellAttribute(cellAttributes, "parent")) ??
    KATANA_EMPTY_DRAWIO_ATTRIBUTES
  );
}

function katanaDrawioSourceObjectAttributes() {
  return new Map(
    Array.from(katanaDrawioRequestSource().matchAll(/<(?:UserObject|object)\b([^>]*)>/g))
      .map((match) => katanaDrawioXmlAttributes(match[1]))
      .map((attributes) => [katanaDrawioCellAttribute(attributes, "id"), attributes])
      .filter((entry) => entry[0]),
  );
}

function katanaDrawioSourceVars() {
  const match = katanaDrawioRequestSource().match(/\bvars='([^']*)'/);
  return match ? katanaDrawioParsedSourceVars(decodeHtmlEntities(match[1])) : new Map();
}

function katanaDrawioParsedSourceVars(source) {
  try {
    return new Map(Object.entries(JSON.parse(source)));
  } catch (_error) {
    return new Map();
  }
}

function katanaDrawioCellAttribute(attributes, name) {
  return katanaDrawioAttributeReader(attributes).get(name) ?? "";
}

function katanaDrawioAttributeReader(attributes) {
  return KATANA_DRAWIO_ATTRIBUTE_READERS[Number(katanaCanReadDrawioAttributes(attributes))](
    attributes,
  );
}

function katanaCanReadDrawioAttributes(attributes) {
  return typeof attributes?.get === "function";
}

function katanaReadableDrawioAttributes(attributes) {
  return attributes;
}

function katanaEmptyDrawioAttributes(_attributes) {
  return KATANA_EMPTY_DRAWIO_ATTRIBUTES;
}

function katanaDrawioXmlAttributes(source) {
  return new Map(
    Array.from(String(source).matchAll(/([a-zA-Z0-9:_-]+)="([^"]*)"/g)).map(
      katanaDrawioXmlAttributeEntry,
    ),
  );
}

function katanaDrawioXmlAttributeEntry(match) {
  return [match[1], decodeHtmlEntities(match[2])];
}

function katanaHasDrawioCellLabel(entry) {
  return [entry.id, entry.label].every(Boolean);
}
