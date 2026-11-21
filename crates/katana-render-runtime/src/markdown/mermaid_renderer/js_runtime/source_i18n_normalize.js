function katanaNormalizeMermaidSourceI18n(source) {
  const context = katanaI18nContext(source);
  const body = context.body.trimStart();
  const type = katanaI18nDiagramType(body);
  const normalizer = KATANA_I18N_SOURCE_NORMALIZERS[type];
  if (!normalizer) {
    return context.result(source);
  }
  return context.result(`${context.prefix}${normalizer(context, context.body)}`);
}
const KATANA_I18N_SOURCE_NORMALIZERS = {
  quadrantChart: katanaNormalizeQuadrantSourceI18n,
};

function katanaNormalizeQuadrantSourceI18n(context, source) {
  return source
    .replace(
      /^(\s*title\s+)(.+?)\s*$/gm,
      (_match, before, text) => `${before}${context.label(text)}`,
    )
    .replace(
      /^(\s*[xy]-axis\s+)(.+?)(\s+-->\s+)(.+?)\s*$/gm,
      (_match, before, left, arrow, right) =>
        `${before}${context.label(left)}${arrow}${context.label(right)}`,
    )
    .replace(
      /^(\s*quadrant-\d+\s+)(.+?)\s*$/gm,
      (_match, before, text) => `${before}${context.label(text)}`,
    )
    .replace(
      /^(\s*)([^:\n]+?)(\s*:\s*\[[^\]]+\]\s*)$/gm,
      (_match, before, text, after) => `${before}${context.label(text)}${after}`,
    );
}
