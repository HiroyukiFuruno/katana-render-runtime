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
  "architecture-beta": katanaNormalizeBracketLabelSourceI18n,
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

function katanaNormalizeBracketLabelSourceI18n(context, source) {
  return source.replace(/\[([^\]\n]+)\]/g, (match, text) =>
    katanaBracketLabelSourceI18nReplacement(context, match, text),
  );
}

function katanaBracketLabelSourceI18nReplacement(context, match, text) {
  return katanaBracketLabelSourceI18nReplacers()[Number(katanaNeedsI18nPlaceholder(text))](
    context,
    match,
    text,
  );
}

function katanaBracketLabelSourceI18nReplacers() {
  return [katanaKeepBracketLabelSourceI18n, katanaReplaceBracketLabelSourceI18n];
}

function katanaKeepBracketLabelSourceI18n(_context, match) {
  return match;
}

function katanaReplaceBracketLabelSourceI18n(context, _match, text) {
  return `[${context.label(text)}]`;
}
