function katanaNormalizeRequirementLocalizedSvg(svg) {
  if (!katanaIsRequirementLocalizedSvg(svg)) {
    return svg;
  }
  return KATANA_REQUIREMENT_LOCALIZED_LAYOUTS.reduce(katanaApplyRequirementLayoutRule, svg);
}

function katanaIsRequirementLocalizedSvg(svg) {
  return svg.includes('aria-roledescription="requirement"');
}

function katanaApplyRequirementLayoutRule(svg, rule) {
  if (!svg.includes(rule.marker)) {
    return svg;
  }
  return rule.replacements.reduce(katanaApplyRequirementLayoutReplacement, svg);
}

function katanaApplyRequirementLayoutReplacement(svg, replacement) {
  return svg.split(replacement[0]).join(replacement[1]);
}

const KATANA_REQUIREMENT_LOCALIZED_LAYOUTS = [
  {
    marker: "テスト要件",
    replacements: [
      ["161.212", "161.21875"],
      ["80.60600000000001", "80.609375"],
      ["80.606", "80.609"],
      ["72.60600000000001", "72.609375"],
      ["69.0515", "69.0546875"],
      ["62.60600000000001", "62.609375"],
      ["59.051500000000004", "59.0546875"],
      ["46.336", "46.3359375"],
      ["20.486517499999998", "21.2265625"],
    ],
  },
  {
    marker: "OSに依存しないランタイム",
    replacements: [
      ["203.081", "203.09375"],
      ["101.5405", "101.546875"],
      ["101.541", "101.547"],
      ["91.79678750000001", "92.0546875"],
      ["81.79678750000001", "82.0546875"],
      ["93.5405", "93.546875"],
      ["83.5405", "83.546875"],
      ["73.8304825", "73.5859375"],
      ["62.60600000000001", "62.609375"],
    ],
  },
];
